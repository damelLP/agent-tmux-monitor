//! End-to-end harness exercising real binaries against a real tmux server.
//!
//! Four scenarios are covered:
//!
//! 1. **Protocol-level e2e**: spawn `atmd` as a child, hand-roll a client over
//!    a Unix socket, register a session via `HookEvent`, observe via
//!    `ListSessions`. This is the wire-protocol regression net.
//!
//! 2. **Real `atm` client**: shell out to the actual `atm` binary
//!    (`atm list --format json`), pointing it at our test daemon via
//!    `ATM_SOCKET`. Asserts the test session is present in the parsed
//!    JSON output. This validates the full client → daemon → JSON path.
//!
//! 3. **Real tmux library against an isolated server**: drive
//!    `atm_tmux::RealTmuxClient::with_socket(label)` against the private
//!    tmux server we spun up. Splits a pane, sends keys, captures output.
//!
//! 4. **`atm spawn` with a `claude` fixture**: run the actual `atm spawn`
//!    subcommand against our isolated tmux server (via `ATM_TMUX_SOCKET`),
//!    using a Python shim named `claude` placed first on PATH. The shim
//!    connects to atmd over its socket, sends a `UserPromptSubmit` hook
//!    event carrying `$TMUX_PANE`, and sleeps to keep the pane alive
//!    (atmd's stale-session reaper runs every 2s). The test then polls
//!    `atm list --format json` until a session whose `tmux_pane` matches
//!    the pane atm spawn created shows up. This validates the entire
//!    spawn → tmux split → hook event → registry update loop.
//!
//! What's still **not** covered (and why):
//!
//! - `atm workspace attach`: requires hijacking the controlling terminal's
//!    tmux state in non-trivial ways. Out of reach without an attach
//!    smoke-test mode or major fixture work.
//!
//! The test skips cleanly (printed message, returns Ok) when `tmux` is
//! absent from PATH, when the binaries weren't built, or — for scenario
//! 4 only — when `python3` is unavailable.
//!
//! Per CLAUDE.md, tests are an explicit `unwrap()`/`expect()`-allowed zone.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use atm_protocol::{ClientMessage, DaemonMessage, ProtocolVersion};
use atm_tmux::{PaneDirection, RealTmuxClient, TmuxClient};
use serde_json::{json, Value};
use tempfile::TempDir;

const SOCKET_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const SOCKET_POLL_INTERVAL: Duration = Duration::from_millis(50);
const SESSION_APPEAR_TIMEOUT: Duration = Duration::from_secs(2);
const SESSION_POLL_INTERVAL: Duration = Duration::from_millis(50);
const DAEMON_SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// Returns `Some(())` if `tmux` is on PATH, `None` otherwise.
fn tmux_on_path() -> Option<()> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        if dir.join("tmux").is_file() {
            return Some(());
        }
    }
    None
}

fn atmd_binary() -> Option<PathBuf> {
    option_env!("CARGO_BIN_EXE_atmd").map(PathBuf::from)
}

fn atm_binary() -> Option<PathBuf> {
    option_env!("CARGO_BIN_EXE_atm").map(PathBuf::from)
}

/// Returns `Some(path)` if `python3` resolves on PATH, `None` otherwise.
fn python3_on_path() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join("python3");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Writes a Python `claude` shim into `dir` and makes it executable.
///
/// The shim impersonates Claude Code's startup just enough to register a
/// session with atmd and stay alive: connect to `$ATM_SOCKET`, send
/// `Connect`, then send a `UserPromptSubmit` `HookEvent` carrying our PID
/// and `$TMUX_PANE`, then sleep. Any error is logged to stderr and the
/// shim exits non-zero so tmux's pane keeps the error visible for
/// debugging.
fn write_claude_shim(dir: &Path) -> PathBuf {
    let path = dir.join("claude");
    let body = r#"#!/usr/bin/env python3
import json, os, socket, sys, time, uuid

socket_path = os.environ.get("ATM_SOCKET", "/tmp/atm.sock")
pane = os.environ.get("TMUX_PANE", "")
session_id = "fixture-" + uuid.uuid4().hex[:12]

try:
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.connect(socket_path)
    f = s.makefile("rwb", buffering=0)

    def send(obj):
        f.write((json.dumps(obj) + "\n").encode())
        f.flush()

    def recv():
        line = f.readline()
        if not line:
            raise RuntimeError("daemon closed connection")
        return json.loads(line.decode())

    send({
        "protocol_version": {"major": 1, "minor": 0},
        "type": "connect",
        "client_id": "claude-fixture",
    })
    msg = recv()
    if msg.get("type") != "connected":
        raise RuntimeError("unexpected handshake response: " + json.dumps(msg))

    send({
        "protocol_version": {"major": 1, "minor": 0},
        "type": "hook_event",
        "data": {
            "session_id": session_id,
            "hook_event_name": "UserPromptSubmit",
            "pid": os.getpid(),
            "tmux_pane": pane,
            "prompt": "fixture probe",
        },
    })

    # Stay alive — atmd's stale-session reaper runs every 2s and would
    # remove this session if the registered PID exited.
    print("[claude-fixture] registered session " + session_id + " for pane " + pane, flush=True)
    time.sleep(60)
except Exception as e:
    print("[claude-fixture] error: " + repr(e), file=sys.stderr, flush=True)
    sys.exit(1)
"#;
    std::fs::write(&path, body).expect("write claude shim");
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(&path).expect("stat shim").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).expect("chmod shim");
    path
}


/// Owns a private tmux server scoped to this test (tmux `-L <socket>`).
///
/// We pass this label to `RealTmuxClient::with_socket(...)` so the project's
/// own tmux library code talks to *this* server, not the developer's default.
///
/// Env vars passed to `start_with_env(...)` propagate into the tmux server's
/// process — and from there into the shells running in panes the server
/// creates. That's the channel by which scenario 4's `claude` shim sees
/// `ATM_SOCKET` and the shim directory on `PATH`.
struct PrivateTmux {
    socket_label: String,
}

impl PrivateTmux {
    fn start_with_env<I, K, V>(env: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<std::ffi::OsStr>,
        V: AsRef<std::ffi::OsStr>,
    {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let socket_label = format!("atm-e2e-{}-{}", std::process::id(), now_nanos);

        let mut cmd = Command::new("tmux");
        for (k, v) in env {
            cmd.env(k, v);
        }
        let status = cmd
            .args(["-L", &socket_label, "new-session", "-d", "-s", "probe"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("spawn tmux new-session");
        assert!(status.success(), "tmux new-session failed");

        Self { socket_label }
    }

    fn label(&self) -> &str {
        &self.socket_label
    }
}

impl Drop for PrivateTmux {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .args(["-L", &self.socket_label, "kill-server"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Owns a child `atmd` process and its sandbox dirs. Drop sends SIGTERM and
/// waits briefly so PID files / sockets are cleaned up.
struct Daemon {
    child: Option<Child>,
    socket_path: PathBuf,
    state_dir: PathBuf,
    _state_dir_guard: TempDir,
    _socket_dir_guard: TempDir,
}

impl Daemon {
    fn spawn(binary: &Path) -> Self {
        let socket_dir_guard = tempfile::tempdir().expect("create socket tempdir");
        let state_dir_guard = tempfile::tempdir().expect("create state tempdir");
        let socket_path = socket_dir_guard.path().join("atmd.sock");
        let state_dir = state_dir_guard.path().to_path_buf();

        let child = Command::new(binary)
            .arg("start")
            .env("ATM_SOCKET", &socket_path)
            .env("XDG_STATE_HOME", &state_dir)
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn atmd child");

        let daemon = Self {
            child: Some(child),
            socket_path,
            state_dir,
            _state_dir_guard: state_dir_guard,
            _socket_dir_guard: socket_dir_guard,
        };

        daemon.wait_for_socket();
        daemon
    }

    fn wait_for_socket(&self) {
        let start = Instant::now();
        while start.elapsed() < SOCKET_WAIT_TIMEOUT {
            if self.socket_path.exists() {
                return;
            }
            std::thread::sleep(SOCKET_POLL_INTERVAL);
        }
        panic!(
            "atmd socket {} did not appear within {:?}",
            self.socket_path.display(),
            SOCKET_WAIT_TIMEOUT
        );
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            unsafe {
                libc::kill(child.id() as i32, libc::SIGTERM);
            }

            let deadline = Instant::now() + DAEMON_SHUTDOWN_GRACE;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => return,
                    Ok(None) if Instant::now() >= deadline => break,
                    Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                    Err(_) => break,
                }
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Synchronous protocol client. Blocking std `UnixStream` keeps the harness
/// readable as a linear "send / recv / assert" script without dragging in a
/// runtime for the protocol-level scenario.
struct Client {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Client {
    fn connect(path: &Path) -> Self {
        let stream = UnixStream::connect(path).expect("connect to atmd socket");
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");
        let writer = stream.try_clone().expect("clone unix stream");
        Self {
            reader: BufReader::new(stream),
            writer,
        }
    }

    fn send(&mut self, msg: ClientMessage) {
        let mut json = serde_json::to_vec(&msg).expect("serialize ClientMessage");
        json.push(b'\n');
        self.writer.write_all(&json).expect("write client message");
        self.writer.flush().expect("flush client message");
    }

    fn recv(&mut self) -> DaemonMessage {
        let mut line = String::new();
        let bytes = self.reader.read_line(&mut line).expect("read daemon line");
        assert!(bytes > 0, "daemon closed connection unexpectedly");
        serde_json::from_str(&line).expect("deserialize DaemonMessage")
    }
}

#[tokio::test(flavor = "current_thread")]
async fn atm_atmd_tmux_end_to_end() {
    if tmux_on_path().is_none() {
        eprintln!("SKIP: tmux not on PATH");
        return;
    }
    let Some(atmd_path) = atmd_binary() else {
        eprintln!("SKIP: CARGO_BIN_EXE_atmd not set");
        return;
    };
    let Some(atm_path) = atm_binary() else {
        eprintln!("SKIP: CARGO_BIN_EXE_atm not set");
        return;
    };

    // Daemon comes up first so the tmux server's env can carry ATM_SOCKET
    // straight into any panes it births. Scenario 4's shim reads it from
    // there to find the daemon socket.
    let daemon = Daemon::spawn(&atmd_path);

    // Stage the `claude` shim. We invoke it by absolute path via
    // ATM_SPAWN_COMMAND, so we don't need to fight the user's shell init
    // for PATH precedence.
    let shim_dir_guard = tempfile::tempdir().expect("create shim tempdir");
    let shim_path = write_claude_shim(shim_dir_guard.path());

    let tmux_server = PrivateTmux::start_with_env([(
        "ATM_SOCKET",
        daemon.socket_path.as_os_str(),
    )]);

    // ====================================================================
    // Scenario 1: protocol-level e2e via hand-rolled client
    // ====================================================================
    let mut client = Client::connect(&daemon.socket_path);

    client.send(ClientMessage::connect(Some("e2e-harness".into())));
    match client.recv() {
        DaemonMessage::Connected {
            protocol_version,
            client_id,
        } => {
            assert_eq!(protocol_version, ProtocolVersion::CURRENT);
            assert_eq!(client_id, "e2e-harness");
        }
        other => panic!("expected Connected, got {other:?}"),
    }

    // Snapshot baseline so we tolerate sessions discovered from /proc.
    client.send(ClientMessage::list_sessions());
    let baseline_count = match client.recv() {
        DaemonMessage::SessionList { sessions } => sessions.len(),
        other => panic!("expected SessionList, got {other:?}"),
    };

    client.send(ClientMessage::discover());
    match client.recv() {
        DaemonMessage::DiscoveryComplete { .. } => {}
        other => panic!("expected DiscoveryComplete, got {other:?}"),
    }

    let session_id = format!(
        "e2e-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let hook_event = json!({
        "session_id": session_id,
        "hook_event_name": "UserPromptSubmit",
        "pid": std::process::id(),
        "prompt": "e2e harness probe",
    });
    client.send(ClientMessage::hook_event(hook_event));

    // Poll until the new session appears.
    let deadline = Instant::now() + SESSION_APPEAR_TIMEOUT;
    let found = loop {
        client.send(ClientMessage::list_sessions());
        let sessions = match client.recv() {
            DaemonMessage::SessionList { sessions } => sessions,
            other => panic!("expected SessionList, got {other:?}"),
        };

        if sessions.iter().any(|s| s.id.as_str() == session_id) {
            assert!(
                sessions.len() >= baseline_count + 1,
                "session count should grow by at least one (baseline={baseline_count}, now={})",
                sessions.len()
            );
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(SESSION_POLL_INTERVAL);
    };
    assert!(
        found,
        "session {session_id} never appeared in registry within {SESSION_APPEAR_TIMEOUT:?}"
    );

    client.send(ClientMessage::disconnect());
    drop(client);

    // ====================================================================
    // Scenario 2: real `atm` binary as the client
    // ====================================================================
    // `atm list --format json` opens its own connection to the daemon. We
    // point it at our test socket via ATM_SOCKET and hand it the same
    // XDG_STATE_HOME so its `ensure_daemon_running` check (which reads the
    // PID file) sees the daemon we already started — preventing it from
    // spawning a second one.
    let output = Command::new(&atm_path)
        .args(["list", "--format", "json"])
        .env("ATM_SOCKET", &daemon.socket_path)
        .env("XDG_STATE_HOME", &daemon.state_dir)
        .output()
        .expect("run atm list");

    assert!(
        output.status.success(),
        "atm list exited with {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = std::str::from_utf8(&output.stdout).expect("utf-8 stdout");
    let parsed: Value = serde_json::from_str(stdout).expect("atm list emits valid JSON");
    let arr = parsed.as_array().expect("atm list emits a JSON array");
    let our_session = arr
        .iter()
        .find(|s| s.get("id").and_then(Value::as_str) == Some(session_id.as_str()));
    assert!(
        our_session.is_some(),
        "real `atm list` JSON output did not contain our session {session_id}; got: {stdout}"
    );

    // ====================================================================
    // Scenario 3: drive the isolated tmux server via atm-tmux library
    // ====================================================================
    // Unlike atmd's discovery code, `RealTmuxClient` supports `-L`, so this
    // exercise stays fully isolated from the developer's default tmux.
    let tmux = RealTmuxClient::with_socket(tmux_server.label().to_string());

    let panes_before = tmux.list_panes().await.expect("list panes (initial)");
    assert!(
        !panes_before.is_empty(),
        "private tmux server should have at least the probe session's pane"
    );
    let probe_pane = panes_before
        .iter()
        .find(|p| p.session_name == "probe")
        .expect("probe session present")
        .pane_id
        .clone();

    let new_pane = tmux
        .split_window(&probe_pane, "30%", PaneDirection::Below, None)
        .await
        .expect("split-window");
    assert!(
        new_pane.starts_with('%'),
        "split-window should return a tmux pane id like '%N', got {new_pane:?}"
    );

    tmux.send_keys(&new_pane, "echo atm-e2e-marker")
        .await
        .expect("send-keys");
    tmux.send_keys(&new_pane, "Enter")
        .await
        .expect("send-keys Enter");

    // Wait for the echo to land in the pane's scrollback. tmux send-keys is
    // asynchronous from the shell's perspective, so this is a true poll.
    let capture_deadline = Instant::now() + Duration::from_secs(3);
    let mut saw_marker = false;
    while Instant::now() < capture_deadline {
        let lines = tmux.capture_pane(&new_pane).await.unwrap_or_default();
        if lines.iter().any(|l| l.contains("atm-e2e-marker")) {
            saw_marker = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        saw_marker,
        "expected 'atm-e2e-marker' to appear in pane {new_pane}'s capture within 3s"
    );

    let panes_after = tmux.list_panes().await.expect("list panes (after split)");
    assert_eq!(
        panes_after.len(),
        panes_before.len() + 1,
        "split-window should add exactly one pane to the private server"
    );

    // ====================================================================
    // Scenario 4: real `atm spawn` driving the isolated tmux server, with
    // a Python `claude` shim registering the new session back to atmd.
    // ====================================================================
    if python3_on_path().is_none() {
        eprintln!("SKIP scenario 4: python3 not on PATH");
        return;
    }

    let panes_pre_spawn = tmux.list_panes().await.expect("list panes (pre-spawn)");
    let probe_pane_pre_spawn = panes_pre_spawn
        .iter()
        .find(|p| p.session_name == "probe")
        .expect("probe pane present")
        .pane_id
        .clone();

    let spawn_output = Command::new(&atm_path)
        .args([
            "spawn",
            "--target-pane",
            &probe_pane_pre_spawn,
            "--direction",
            "below",
            "--size",
            "20%",
        ])
        // is_in_tmux() just checks for the env var; any value passes.
        .env("TMUX", "atm-e2e-fixture-no-real-tmux-here")
        // Redirect every tmux invocation atm spawn makes onto our server.
        .env("ATM_TMUX_SOCKET", tmux_server.label())
        // Override the hardcoded "claude" command with our shim, by
        // absolute path so shell PATH precedence is irrelevant.
        .env("ATM_SPAWN_COMMAND", &shim_path)
        // The shim talks back to *our* daemon over this socket.
        .env("ATM_SOCKET", &daemon.socket_path)
        // ensure_daemon_running reads PID file under XDG_STATE_HOME.
        .env("XDG_STATE_HOME", &daemon.state_dir)
        .output()
        .expect("run atm spawn");

    assert!(
        spawn_output.status.success(),
        "atm spawn exited with {:?}\nstdout: {}\nstderr: {}",
        spawn_output.status.code(),
        String::from_utf8_lossy(&spawn_output.stdout),
        String::from_utf8_lossy(&spawn_output.stderr)
    );

    // Identify the pane atm spawn just created on our server.
    let panes_post_spawn = tmux.list_panes().await.expect("list panes (post-spawn)");
    let new_panes: Vec<_> = panes_post_spawn
        .iter()
        .filter(|p| !panes_pre_spawn.iter().any(|q| q.pane_id == p.pane_id))
        .collect();
    assert_eq!(
        new_panes.len(),
        1,
        "atm spawn should create exactly one new pane; got {} ({:?})",
        new_panes.len(),
        new_panes.iter().map(|p| &p.pane_id).collect::<Vec<_>>()
    );
    let spawned_pane = new_panes[0].pane_id.clone();

    // Poll `atm list --format json` until our fixture-minted session shows
    // up. We can't filter on tmux_pane alone: pane IDs (`%N`) aren't unique
    // across tmux servers, and scenario 1's `Discover` may have surfaced
    // user-side claude processes whose tmux_pane (resolved against the
    // *default* server) coincidentally collides with our spawned pane in
    // the *private* server. The "fixture-" prefix is uniquely ours, so we
    // gate on that AND verify the tmux_pane match for safety.
    //
    // Generous deadline because the shim has to: (a) wait for tmux's
    // send-keys to land, (b) start the Python interpreter, (c) connect to
    // atmd, (d) hand off the hook event, (e) atmd has to apply it.
    let spawn_deadline = Instant::now() + Duration::from_secs(10);
    let mut fixture_session: Option<Value> = None;
    while Instant::now() < spawn_deadline {
        let list_output = Command::new(&atm_path)
            .args(["list", "--format", "json"])
            .env("ATM_SOCKET", &daemon.socket_path)
            .env("XDG_STATE_HOME", &daemon.state_dir)
            .output()
            .expect("run atm list");

        if list_output.status.success() {
            if let Ok(parsed) = serde_json::from_slice::<Value>(&list_output.stdout) {
                if let Some(arr) = parsed.as_array() {
                    if let Some(s) = arr.iter().find(|s| {
                        s.get("id")
                            .and_then(Value::as_str)
                            .map_or(false, |id| id.starts_with("fixture-"))
                    }) {
                        fixture_session = Some(s.clone());
                        break;
                    }
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }

    let session = match fixture_session {
        Some(s) => s,
        None => {
            // Snapshot what's in the spawned pane so the failure is
            // diagnosable instead of mysterious.
            let pane_dump = tmux
                .capture_pane(&spawned_pane)
                .await
                .unwrap_or_else(|e| vec![format!("(capture-pane failed: {e})")]);
            panic!(
                "no session with id starting with 'fixture-' appeared within 10s.\n\
                 atm spawn stdout: {}\n\
                 atm spawn stderr: {}\n\
                 spawned pane ({spawned_pane}) contents:\n{}",
                String::from_utf8_lossy(&spawn_output.stdout),
                String::from_utf8_lossy(&spawn_output.stderr),
                pane_dump.join("\n")
            );
        }
    };

    // The shim records `$TMUX_PANE`, which inside the spawned pane equals
    // the id atm spawn assigned it. This is what closes the loop: spawn
    // created the pane, the pane ran the shim, the shim told atmd, atmd
    // remembered the pane id we know — full e2e for the spawn flow.
    let registered_pane = session
        .get("tmux_pane")
        .and_then(Value::as_str)
        .expect("fixture session should carry tmux_pane");
    assert_eq!(
        registered_pane, spawned_pane,
        "fixture session's tmux_pane should match the pane atm spawn created"
    );

    // RAII: tmux server killed by PrivateTmux::drop (which also reaps the
    // shim's pane), daemon killed by Daemon::drop, shim_dir_guard cleaned up.
}
