//! End-to-end harness exercising real binaries against a real tmux server.
//!
//! Three scenarios are covered:
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
//! What's still **not** covered (and why):
//!
//! - `atm spawn` end-to-end requires `claude` on PATH and meaningful TMUX
//!   env state. The pane would be created, but the agent it tries to run
//!   would never register with atmd. Stubbing that out reliably is a
//!   bigger lift than this harness aims for.
//! - `atm workspace attach` requires the test process to behave as if it
//!   were already inside a tmux session — touchable, but a separate
//!   concern from the protocol/library coverage above.
//!
//! The test skips cleanly (printed message, returns Ok) when `tmux` is
//! absent from PATH or the binaries weren't built.
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

/// Owns a private tmux server scoped to this test (tmux `-L <socket>`).
///
/// We pass this label to `RealTmuxClient::with_socket(...)` so the project's
/// own tmux library code talks to *this* server, not the developer's default.
struct PrivateTmux {
    socket_label: String,
}

impl PrivateTmux {
    fn start() -> Self {
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let socket_label = format!("atm-e2e-{}-{}", std::process::id(), now_nanos);

        let status = Command::new("tmux")
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

    let tmux_server = PrivateTmux::start();
    let daemon = Daemon::spawn(&atmd_path);

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

    // RAII: tmux server killed by PrivateTmux::drop, daemon killed by Daemon::drop.
}
