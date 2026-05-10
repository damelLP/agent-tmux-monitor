//! Session discovery - finds existing coding-agent sessions.
//!
//! Scans `/proc` for running agent processes and registers them with
//! minimal data. Each process is tested against per-harness detectors
//! (Claude Code, pi, future) and the matching `Harness` is recorded so
//! the registry tags the session correctly. Full session data arrives
//! via status-line updates (Claude) or extension events (pi).
//!
//! # Async Safety
//!
//! All filesystem operations are run via `spawn_blocking` to avoid
//! blocking the async runtime.
//!
//! # Panic-Free Guarantees
//!
//! This module follows CLAUDE.md panic-free policy:
//! - No `.unwrap()`, `.expect()`, `panic!()`, `unreachable!()`, `todo!()`
//! - All fallible operations use `?`, pattern matching, or `unwrap_or`
//! - Discovery errors are logged but never fatal

use std::path::PathBuf;

use atm_core::{Harness, SessionId};
use thiserror::Error;
use tracing::{debug, info, trace, warn};

use crate::registry::RegistryHandle;
use crate::tmux::find_pane_for_pid;

// ============================================================================
// Constants
// ============================================================================

/// Default maximum age of a transcript file to be considered "active" (60 seconds).
pub const DEFAULT_TRANSCRIPT_MAX_AGE_SECS: u64 = 60;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during discovery.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    /// Failed to read /proc directory
    #[error("failed to read /proc: {0}")]
    ProcReadError(String),

    /// Failed to read process information
    #[error("failed to read process {pid}: {message}")]
    ProcessReadError { pid: u32, message: String },

    /// No active transcript found
    #[error("no active transcript found for PID {0}")]
    NoActiveTranscript(u32),

    /// Registry error during registration
    #[error("registry error: {0}")]
    RegistryError(String),
}

// ============================================================================
// Result Type
// ============================================================================

/// Result of a discovery operation.
#[derive(Debug, Clone, Default)]
pub struct DiscoveryResult {
    /// Number of sessions successfully discovered
    pub discovered: u32,
    /// Number of failures during discovery
    pub failed: u32,
}

// ============================================================================
// Discovered Process
// ============================================================================

/// Information about a running agent process discovered via /proc.
///
/// `harness` records which detector matched — used by the registry to
/// tag the session so the TUI shows the right vendor badge from the
/// first frame, before any adapter event arrives.
#[derive(Debug, Clone)]
struct DiscoveredProcess {
    /// Process ID
    pid: u32,
    /// Working directory
    cwd: PathBuf,
    /// Tmux pane ID if running in tmux
    tmux_pane: Option<String>,
    /// Which coding-agent harness this process belongs to.
    harness: Harness,
}

// ============================================================================
// Discovery Service
// ============================================================================

/// Service for discovering existing Claude Code sessions.
///
/// Scans `/proc` for Claude processes and registers them in the registry.
/// Called on daemon startup and when TUI requests a rescan.
pub struct DiscoveryService {
    registry: RegistryHandle,
    /// Maximum age in seconds for a transcript to be considered "active".
    transcript_max_age_secs: u64,
}

impl DiscoveryService {
    /// Creates a new discovery service with default settings.
    #[must_use]
    pub fn new(registry: RegistryHandle) -> Self {
        Self {
            registry,
            transcript_max_age_secs: DEFAULT_TRANSCRIPT_MAX_AGE_SECS,
        }
    }

    /// Creates a new discovery service with custom transcript age threshold.
    #[must_use]
    pub fn with_max_age(registry: RegistryHandle, transcript_max_age_secs: u64) -> Self {
        Self {
            registry,
            transcript_max_age_secs,
        }
    }

    /// Discover and register existing Claude sessions.
    ///
    /// Scans `/proc` for Claude processes, finds their transcripts,
    /// and registers minimal sessions in the registry.
    ///
    /// # Returns
    ///
    /// A `DiscoveryResult` with counts of discovered and failed sessions.
    /// Errors for individual sessions are logged but don't stop discovery.
    pub async fn discover(&self) -> DiscoveryResult {
        let mut result = DiscoveryResult::default();

        // Scan for agent processes (blocking I/O in spawn_blocking)
        let processes = match tokio::task::spawn_blocking(scan_agent_processes).await {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => {
                warn!(error = %e, "Failed to scan for agent processes");
                return result;
            }
            Err(e) => {
                warn!(error = %e, "Discovery task panicked");
                return result;
            }
        };

        if processes.is_empty() {
            debug!("No agent processes found");
            return result;
        }

        debug!(count = processes.len(), "Found agent processes");

        // Try to discover each process
        let max_age_secs = self.transcript_max_age_secs;
        for process in processes {
            match self.discover_session(&process, max_age_secs).await {
                Ok(Some(session_id)) => {
                    debug!(
                        session_id = %session_id,
                        pid = process.pid,
                        "Discovered session"
                    );
                    result.discovered += 1;
                }
                Ok(None) => {
                    // Session already registered or no transcript found
                    debug!(
                        pid = process.pid,
                        "Skipped process (already registered or no transcript)"
                    );
                }
                Err(e) => {
                    debug!(
                        pid = process.pid,
                        error = %e,
                        "Failed to discover session"
                    );
                    result.failed += 1;
                }
            }
        }

        if result.discovered > 0 || result.failed > 0 {
            info!(
                discovered = result.discovered,
                failed = result.failed,
                "Discovery complete"
            );
        }

        result
    }

    /// Discovers a session for a Claude process.
    ///
    /// Always registers a pending session with ID `pending-{pid}`. The real
    /// session_id will be set when the first status line update arrives
    /// (which includes both session_id and pid).
    ///
    /// We intentionally avoid using transcript filenames as session IDs because:
    /// - Multiple Claude processes in the same directory share the same transcript folder
    /// - We cannot reliably map a transcript file to a specific PID
    /// - Using transcript-based IDs caused session deduplication bugs
    ///
    /// Returns:
    /// - `Ok(Some(session_id))` if session was discovered and registered
    /// - `Ok(None)` if session already exists
    /// - `Err` if registration failed
    async fn discover_session(
        &self,
        process: &DiscoveredProcess,
        #[allow(unused_variables)] max_age_secs: u64,
    ) -> Result<Option<SessionId>, DiscoveryError> {
        let pid = process.pid;
        let cwd = process.cwd.clone();
        let tmux_pane = process.tmux_pane.clone();
        let harness = process.harness;

        // Always use pending-{pid} as the initial session ID.
        // The real session_id will arrive via status line update or
        // adapter event.
        let session_id = SessionId::pending_from_pid(pid);

        debug!(
            pid,
            session_id = %session_id,
            tmux_pane = ?tmux_pane,
            harness = %harness,
            "Creating pending session for discovered agent process"
        );

        // Register the discovered session
        match self
            .registry
            .register_discovered(session_id.clone(), pid, cwd, tmux_pane, harness)
            .await
        {
            Ok(()) => Ok(Some(session_id)),
            Err(e) => Err(DiscoveryError::RegistryError(e.to_string())),
        }
    }
}

// ============================================================================
// Blocking Filesystem Operations
// ============================================================================

/// Scans /proc for coding-agent processes.
///
/// Single pass: for each PID, dispatches through per-harness detectors
/// (Claude → pi → …). The first detector that matches wins. Adding a
/// new harness means adding one detector below; no caller changes.
///
/// This function performs blocking I/O and should be called via
/// `spawn_blocking`.
fn scan_agent_processes() -> Result<Vec<DiscoveredProcess>, DiscoveryError> {
    let mut processes = Vec::new();

    // Read /proc directory
    let proc_dir =
        std::fs::read_dir("/proc").map_err(|e| DiscoveryError::ProcReadError(e.to_string()))?;

    for entry in proc_dir.flatten() {
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();

        // Only process numeric directories (PIDs)
        let pid: u32 = match name.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        if let Some(process) = detect_agent_process(pid) {
            processes.push(process);
        }
    }

    Ok(processes)
}

/// Tries each registered harness detector against `pid`. Returns the
/// first match or `None`.
///
/// Order matters insofar as we want common, fast checks first. Both
/// `check_claude_process` and `check_pi_process` walk `/proc/{pid}/exe`
/// then fall back to cmdline; cost is similar. Claude is checked first
/// only because it's the historic default.
fn detect_agent_process(pid: u32) -> Option<DiscoveredProcess> {
    if let Some(p) = check_claude_process(pid) {
        return Some(p);
    }
    if let Some(p) = check_pi_process(pid) {
        return Some(p);
    }
    None
}

/// Checks if a path string represents a Claude executable.
///
/// Matches:
/// - `/path/to/claude` (ends with /claude)
/// - `claude` (bare command name)
/// - `~/.local/share/claude/versions/X.Y.Z/...` (versioned installs)
fn is_claude_path(path: &str) -> bool {
    path.ends_with("/claude") || path == "claude" || path.contains("claude/versions/")
}

/// Checks if a PID is a Claude Code process.
///
/// First attempts to identify via `/proc/{pid}/exe`. Falls back to
/// `/proc/{pid}/cmdline` for wrapper scripts.
fn check_claude_process(pid: u32) -> Option<DiscoveredProcess> {
    if let Some(process) = check_via_exe(pid, is_claude_path, Harness::ClaudeCode) {
        return Some(process);
    }

    let result = check_via_cmdline(pid, is_claude_path, Harness::ClaudeCode);

    if result.is_some() {
        trace!(
            pid,
            "Detected Claude via cmdline fallback (exe check failed)"
        );
    }

    result
}

/// Checks if a PID is a `pi` (https://pi.dev/) process.
///
/// `pi` is installed as a node-shebang script — `comm` reports
/// `node` rather than `pi` across most setups, so cmdline is the
/// authoritative match. Recipe (`pgrep -fn 'pi-coding-agent|/bin/pi$'`)
/// is encoded as `is_pi_path`.
fn check_pi_process(pid: u32) -> Option<DiscoveredProcess> {
    if let Some(process) = check_via_exe(pid, is_pi_path, Harness::Pi) {
        return Some(process);
    }
    let result = check_via_cmdline(pid, is_pi_path, Harness::Pi);
    if result.is_some() {
        trace!(pid, "Detected pi via cmdline fallback (exe check failed)");
    }
    result
}

/// Checks if a path string represents a pi executable.
///
/// Matches:
/// - `/path/to/bin/pi` (canonical install)
/// - `*/pi` (any path ending with `/pi` — covers non-`bin` install locations)
/// - `.../pi-coding-agent/...` (npm package path; pi is published as
///   `@mariozechner/pi-coding-agent`)
///
/// Bare `"pi"` is intentionally not matched: `check_via_cmdline`
/// scans every non-flag argv entry, so a bare-string match would
/// false-positive on any process that happens to take `pi` as an
/// argument. Real pi invocations land in argv as either an absolute
/// path or under the published npm package prefix.
fn is_pi_path(path: &str) -> bool {
    path.ends_with("/bin/pi") || path.ends_with("/pi") || path.contains("pi-coding-agent")
}

/// Generic helper: tests `/proc/{pid}/exe` against `path_matches` and
/// returns a `DiscoveredProcess` tagged with `harness` on match.
fn check_via_exe(
    pid: u32,
    path_matches: fn(&str) -> bool,
    harness: Harness,
) -> Option<DiscoveredProcess> {
    let exe_path = format!("/proc/{pid}/exe");
    let exe = std::fs::read_link(&exe_path).ok()?;
    let exe_str = exe.to_string_lossy();

    if !path_matches(&exe_str) {
        return None;
    }

    get_process_info(pid, harness)
}

/// Generic helper: scans `/proc/{pid}/cmdline` arguments and returns
/// a `DiscoveredProcess` tagged with `harness` if any non-flag arg
/// satisfies `path_matches`.
fn check_via_cmdline(
    pid: u32,
    path_matches: fn(&str) -> bool,
    harness: Harness,
) -> Option<DiscoveredProcess> {
    let cmdline_path = format!("/proc/{pid}/cmdline");
    let cmdline_bytes = std::fs::read(&cmdline_path).ok()?;

    let matched = cmdline_bytes
        .split(|&b| b == 0)
        .filter_map(|bytes| std::str::from_utf8(bytes).ok())
        .filter(|s| !s.is_empty())
        .any(|arg| {
            // Skip flag arguments (e.g. --config)
            if arg.starts_with('-') {
                return false;
            }
            path_matches(arg)
        });

    if !matched {
        return None;
    }

    get_process_info(pid, harness)
}

/// Gets process info (cwd, tmux pane) for a PID.
fn get_process_info(pid: u32, harness: Harness) -> Option<DiscoveredProcess> {
    // Read working directory
    let cwd_path = format!("/proc/{pid}/cwd");
    let cwd = std::fs::read_link(&cwd_path).ok()?;

    // Try to find tmux pane for this process
    let tmux_pane = find_pane_for_pid(pid);

    Some(DiscoveredProcess {
        pid,
        cwd,
        tmux_pane,
        harness,
    })
}

// ============================================================================
// Helper Functions (test-only, no longer used in production)
// ============================================================================

#[cfg(test)]
use std::path::Path;
#[cfg(test)]
use std::time::{Duration, SystemTime};

/// Maps a working directory to Claude's project directory.
///
/// Claude stores transcripts in `~/.claude/projects/{escaped-path}/`.
/// The path is escaped by replacing `/` with `-`.
///
/// Example: `/home/user/code/project` -> `~/.claude/projects/-home-user-code-project/`
///
/// Note: This function is no longer used in production discovery.
/// We now always use pending-{pid} and let status line updates provide the real session ID.
/// Kept for tests and potential future use.
#[cfg(test)]
fn cwd_to_project_dir(cwd: &Path) -> PathBuf {
    let escaped = cwd.to_string_lossy().replace('/', "-");

    // Get home directory from HOME environment variable
    let home = std::env::var("HOME")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));

    home.join(".claude/projects").join(escaped)
}

/// Finds the most recently modified transcript in a project directory.
///
/// Only considers:
/// - Files with `.jsonl` extension
/// - Files with UUID-like names (not `agent-*.jsonl` subagent transcripts)
/// - Files modified within the specified max age
///
/// This function performs blocking I/O and should be called via `spawn_blocking`.
///
/// Note: This function is no longer used in production discovery.
/// We now always use pending-{pid} and let status line updates provide the real session ID.
/// Kept for tests and potential future use.
#[cfg(test)]
fn find_active_transcript(project_dir: &Path, max_age_secs: u64) -> Option<PathBuf> {
    let now = SystemTime::now();
    let max_age = Duration::from_secs(max_age_secs);

    let entries = std::fs::read_dir(project_dir).ok()?;

    let mut candidates: Vec<(PathBuf, SystemTime)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();

            // Must be a .jsonl file
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                return None;
            }

            // Must be a UUID-like name (not agent-*)
            let stem = path.file_stem()?.to_string_lossy();
            if stem.starts_with("agent-") {
                return None;
            }

            // Check modification time
            let metadata = entry.metadata().ok()?;
            let mtime = metadata.modified().ok()?;

            // Must be modified within max_age
            let age = now.duration_since(mtime).ok()?;
            if age > max_age {
                return None;
            }

            Some((path, mtime))
        })
        .collect();

    // Sort by modification time (most recent first)
    candidates.sort_by_key(|c| std::cmp::Reverse(c.1));

    // Return the most recent
    candidates.into_iter().next().map(|(path, _)| path)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use std::time::Duration;
    use tempfile::TempDir;

    #[test]
    fn test_cwd_to_project_dir_simple() {
        let cwd = Path::new("/home/user/code/project");
        let project_dir = cwd_to_project_dir(cwd);

        let expected_suffix = ".claude/projects/-home-user-code-project";
        assert!(
            project_dir.to_string_lossy().ends_with(expected_suffix),
            "Expected path to end with '{}', got '{}'",
            expected_suffix,
            project_dir.display()
        );
    }

    #[test]
    fn test_cwd_to_project_dir_root() {
        let cwd = Path::new("/");
        let project_dir = cwd_to_project_dir(cwd);

        // Root path becomes empty after escaping, so just check it ends with projects/
        assert!(project_dir.to_string_lossy().contains(".claude/projects"));
    }

    #[test]
    fn test_cwd_to_project_dir_nested() {
        let cwd = Path::new("/home/user/very/deeply/nested/project");
        let project_dir = cwd_to_project_dir(cwd);

        let expected_suffix = "-home-user-very-deeply-nested-project";
        assert!(
            project_dir.to_string_lossy().ends_with(expected_suffix),
            "Got: {}",
            project_dir.display()
        );
    }

    #[test]
    fn test_find_active_transcript_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let result = find_active_transcript(temp_dir.path(), DEFAULT_TRANSCRIPT_MAX_AGE_SECS);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_active_transcript_no_jsonl() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("test.txt"), "not jsonl").unwrap();
        let result = find_active_transcript(temp_dir.path(), DEFAULT_TRANSCRIPT_MAX_AGE_SECS);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_active_transcript_ignores_agent_files() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("agent-abc123.jsonl"), "{}").unwrap();
        let result = find_active_transcript(temp_dir.path(), DEFAULT_TRANSCRIPT_MAX_AGE_SECS);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_active_transcript_finds_recent() {
        let temp_dir = TempDir::new().unwrap();
        let session_file = temp_dir
            .path()
            .join("226f3c14-cc34-4118-804b-b7d442aa2363.jsonl");
        fs::write(&session_file, "{}").unwrap();

        let result = find_active_transcript(temp_dir.path(), DEFAULT_TRANSCRIPT_MAX_AGE_SECS);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), session_file);
    }

    #[test]
    fn test_find_active_transcript_picks_most_recent() {
        let temp_dir = TempDir::new().unwrap();

        // Create two files with different modification times
        let older = temp_dir
            .path()
            .join("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee.jsonl");
        let newer = temp_dir
            .path()
            .join("ffffffff-0000-1111-2222-333333333333.jsonl");

        fs::write(&older, "old").unwrap();
        // Small delay to ensure different mtime
        std::thread::sleep(Duration::from_millis(10));
        fs::write(&newer, "new").unwrap();

        let result = find_active_transcript(temp_dir.path(), DEFAULT_TRANSCRIPT_MAX_AGE_SECS);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), newer);
    }

    #[test]
    fn test_find_active_transcript_respects_custom_max_age() {
        let temp_dir = TempDir::new().unwrap();
        let session_file = temp_dir
            .path()
            .join("226f3c14-cc34-4118-804b-b7d442aa2363.jsonl");
        fs::write(&session_file, "{}").unwrap();

        // With a very short max age (0 seconds), file should not be found
        // (file was just created, so mtime age > 0)
        // Use 1ms sleep to ensure file is "old"
        std::thread::sleep(Duration::from_millis(1));
        let result = find_active_transcript(temp_dir.path(), 0);
        assert!(result.is_none());

        // With default max age, file should be found
        let result = find_active_transcript(temp_dir.path(), DEFAULT_TRANSCRIPT_MAX_AGE_SECS);
        assert!(result.is_some());
    }

    #[test]
    fn test_discovery_result_default() {
        let result = DiscoveryResult::default();
        assert_eq!(result.discovered, 0);
        assert_eq!(result.failed, 0);
    }

    // ========================================================================
    // Tests for is_claude_path helper
    // ========================================================================

    #[test]
    fn test_is_claude_path_absolute_path() {
        assert!(is_claude_path("/usr/local/bin/claude"));
        assert!(is_claude_path("/home/user/.local/bin/claude"));
    }

    #[test]
    fn test_is_claude_path_bare_command() {
        assert!(is_claude_path("claude"));
    }

    #[test]
    fn test_is_claude_path_versioned_install() {
        assert!(is_claude_path(
            "/home/user/.local/share/claude/versions/1.2.3/claude"
        ));
        assert!(is_claude_path("~/.local/share/claude/versions/0.5.0/node"));
    }

    #[test]
    fn test_is_claude_path_rejects_non_claude() {
        assert!(!is_claude_path("/usr/bin/bash"));
        assert!(!is_claude_path("vim"));
        assert!(!is_claude_path("/home/user/claudette")); // not ending with /claude
        assert!(!is_claude_path("claude-dev")); // not exact match
    }

    #[test]
    fn test_is_claude_path_edge_cases() {
        // Path that contains "claude" but not at the end or in versions
        assert!(!is_claude_path("/home/claudeuser/bin/tool"));
        // Empty string
        assert!(!is_claude_path(""));
    }
}
