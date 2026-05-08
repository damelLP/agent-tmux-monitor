//! End-to-end harness exercising a real `atmd` child process against a real
//! `tmux` server, driving it through the wire protocol over a Unix socket.
//!
//! Unlike the in-process tests under `crates/atmd/tests/`, this spawns the
//! actual `atmd` binary (via `CARGO_BIN_EXE_atmd`, which is only injected for
//! tests in the package that declares the `[[bin]]` — the workspace root).
//!
//! The test is gated on `tmux` being on PATH and on the binary being available;
//! both checks short-circuit with a printed skip rather than failing, so the
//! suite stays green on minimal CI images.
//!
//! Per CLAUDE.md, tests are an explicit `unwrap()`/`expect()`-allowed zone.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use atm_protocol::{ClientMessage, DaemonMessage, ProtocolVersion};
use serde_json::json;
use tempfile::TempDir;

const SOCKET_WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const SOCKET_POLL_INTERVAL: Duration = Duration::from_millis(50);
const SESSION_APPEAR_TIMEOUT: Duration = Duration::from_secs(2);
const SESSION_POLL_INTERVAL: Duration = Duration::from_millis(50);
const DAEMON_SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// Returns `Some(PathBuf)` if `tmux` is on PATH, `None` otherwise.
fn tmux_on_path() -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join("tmux");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

/// Returns the `atmd` binary path, or `None` if Cargo didn't inject one.
fn atmd_binary() -> Option<PathBuf> {
    option_env!("CARGO_BIN_EXE_atmd").map(PathBuf::from)
}

/// Owns a private tmux server scoped to this test (tmux `-L <socket>`).
///
/// NOTE: atmd's tmux invocations don't pass `-L`, so this server doesn't
/// actually intercept atmd's calls — it just guarantees a server is alive
/// somewhere so `tmux list-panes -a` doesn't fail outright. Real isolation
/// would require a code change in `crates/atmd/src/tmux.rs`.
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
    _state_dir: TempDir,
    _socket_dir: TempDir,
}

impl Daemon {
    fn spawn(binary: &Path) -> Self {
        let socket_dir = tempfile::tempdir().expect("create socket tempdir");
        let state_dir = tempfile::tempdir().expect("create state tempdir");
        let socket_path = socket_dir.path().join("atmd.sock");

        let child = Command::new(binary)
            .arg("start")
            .env("ATM_SOCKET", &socket_path)
            .env("XDG_STATE_HOME", state_dir.path())
            // Quiet the daemon's tracing output during tests.
            .env("RUST_LOG", "warn")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn atmd child");

        let daemon = Self {
            child: Some(child),
            socket_path,
            _state_dir: state_dir,
            _socket_dir: socket_dir,
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
            // Polite SIGTERM so atmd can clean up its PID file & socket.
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

/// Synchronous protocol client tailored for this harness. We use std's
/// blocking `UnixStream` here rather than tokio so the test itself doesn't
/// need a runtime — keeping the harness as close to "what an external
/// observer sees" as possible.
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

#[test]
fn atm_atmd_tmux_end_to_end() {
    let Some(_tmux) = tmux_on_path() else {
        eprintln!("SKIP: tmux not on PATH");
        return;
    };
    let Some(atmd_path) = atmd_binary() else {
        eprintln!("SKIP: CARGO_BIN_EXE_atmd not set; nothing to drive");
        return;
    };

    // Hold the private tmux server alive for the duration of the test so
    // atmd's `tmux list-panes -a` has a server to talk to (even though it
    // hits the default socket, having any server up keeps the call viable
    // in environments where the user has none).
    let _tmux_server = PrivateTmux::start();

    let daemon = Daemon::spawn(&atmd_path);
    let mut client = Client::connect(&daemon.socket_path);

    // ---- 1. Handshake ----
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

    // ---- 2. Baseline session count ----
    // We can't assume zero sessions on a developer machine: atmd runs
    // /proc-based discovery on startup and may pick up real Claude Code
    // processes. Instead, snapshot the count and assert deltas.
    client.send(ClientMessage::list_sessions());
    let baseline_count = match client.recv() {
        DaemonMessage::SessionList { sessions } => sessions.len(),
        other => panic!("expected SessionList, got {other:?}"),
    };

    // ---- 3. Discover round-trip (exercises tmux list-panes path) ----
    client.send(ClientMessage::discover());
    match client.recv() {
        DaemonMessage::DiscoveryComplete { .. } => {}
        other => panic!("expected DiscoveryComplete, got {other:?}"),
    }

    // ---- 4. Register a session via HookEvent ----
    // The registry creates a session when a hook event arrives carrying a
    // non-zero PID. We send our own PID, which is guaranteed to be live.
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

    // ---- 5. Poll until the new session appears ----
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
                "expected session count to grow by at least one: baseline={baseline_count}, now={}",
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

    // ---- 6. Disconnect cleanly ----
    client.send(ClientMessage::disconnect());

    // Daemon's Drop sends SIGTERM and waits for exit; tmux server's Drop
    // tears down the private server. Nothing more to do here.
}
