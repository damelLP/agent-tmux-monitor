//! Setup for ATM integrations across coding-agent harnesses.
//!
//! Detects which harnesses are installed (Claude Code, pi, future) and
//! wires the matching hook for each:
//!
//! - **Claude Code**: writes the `atm-hook` bash script to
//!   `~/.local/bin/`, then registers it in `~/.claude/settings.json`'s
//!   `hooks` and `statusLine` blocks.
//! - **pi** (<https://pi.dev/>): writes the `pi-atm` TypeScript
//!   extension to `~/.pi/packages/pi-atm/`, then adds
//!   `"packages/pi-atm"` to `~/.pi/agent/settings.json`'s `packages`
//!   array. Mirrors how pi-amplike documents local-dev installs.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{json, Value};

/// The atm-hook bash script content (Claude Code), embedded at compile time.
const ATM_HOOK_SCRIPT: &str = include_str!("../scripts/atm-hook");

/// The pi-atm TypeScript extension content, embedded at compile time.
/// pi loads `.ts` files directly via `@mariozechner/jiti`.
const PI_ATM_EXTENSION: &str = include_str!("../../../extensions/pi-atm/extensions/pi-atm.ts");

/// Package manifest written next to the embedded extension. Pi looks
/// at the `pi.extensions` array (not `main`) to discover extension
/// files within an installed package.
const PI_ATM_PACKAGE_JSON: &str = include_str!("../../../extensions/pi-atm/package.json");

/// All valid Claude Code hook types.
/// See: https://docs.anthropic.com/en/docs/claude-code/hooks
const HOOK_TYPES: &[&str] = &[
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "UserPromptSubmit",
    "SessionStart",
    "SessionEnd",
    "Stop",
    "SubagentStart",
    "SubagentStop",
    "PreCompact",
    "PermissionRequest",
];

/// Returns the path to Claude Code settings.json
fn claude_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("settings.json"))
}

/// Returns the path to the atm-hook script
fn hook_script_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".local").join("bin").join("atm-hook"))
        .unwrap_or_else(|| PathBuf::from("/usr/local/bin/atm-hook"))
}

/// Reads a JSON file at `path`, returning an empty object if the file
/// does not exist. Errors carry the path for diagnostics.
fn read_json_file_or_empty(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }

    let content =
        fs::read_to_string(path).with_context(|| format!("Failed to read {}", path.display()))?;

    serde_json::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))
}

/// Writes `value` to `path` as pretty-printed JSON, creating parent
/// directories as needed. Errors carry the path for diagnostics.
fn write_json_file_pretty(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let content = serde_json::to_string_pretty(value)?;
    fs::write(path, content).with_context(|| format!("Failed to write {}", path.display()))
}

/// Reads Claude Code settings, returns empty object if file doesn't exist
fn read_settings() -> Result<Value> {
    let path = claude_settings_path().context("Could not determine home directory")?;
    read_json_file_or_empty(&path)
}

/// Writes Claude Code settings
fn write_settings(settings: &Value) -> Result<()> {
    let path = claude_settings_path().context("Could not determine home directory")?;
    write_json_file_pretty(&path, settings)
}

/// Creates the statusLine configuration entry.
///
/// Uses the same atm-hook script which auto-detects message type.
fn create_status_line_entry() -> Value {
    let hook_path = hook_script_path();
    let command = hook_path.to_string_lossy().to_string();

    json!({
        "type": "command",
        "command": command
    })
}

/// Checks if atm-hook is configured for statusLine
fn has_atm_status_line(status_line: &Value) -> bool {
    status_line
        .get("command")
        .and_then(|c| c.as_str())
        .map(|cmd| cmd.contains("atm-hook"))
        .unwrap_or(false)
}

/// Creates a hook entry for the given hook type.
///
/// Hook types that filter by tool name use a matcher, others don't.
fn create_hook_entry(hook_type: &str) -> Value {
    let hook_path = hook_script_path();
    let command = hook_path.to_string_lossy().to_string();

    // Tool-related hooks use a matcher to filter by tool name
    let needs_matcher = matches!(
        hook_type,
        "PreToolUse" | "PostToolUse" | "PostToolUseFailure" | "PermissionRequest"
    );

    if needs_matcher {
        json!({
            "matcher": "*",
            "hooks": [{
                "type": "command",
                "command": command
            }]
        })
    } else {
        // Session/lifecycle hooks don't use a matcher
        json!({
            "hooks": [{
                "type": "command",
                "command": command
            }]
        })
    }
}

/// Checks if atm hooks are already installed for a hook type
fn has_atm_hook(hooks_array: &[Value]) -> bool {
    hooks_array.iter().any(|entry| {
        entry
            .get("hooks")
            .and_then(|h| h.as_array())
            .map(|hooks| {
                hooks.iter().any(|hook| {
                    hook.get("command")
                        .and_then(|c| c.as_str())
                        .map(|cmd| cmd.contains("atm-hook"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    })
}

/// Removes atm hooks from a hooks array
fn remove_atm_hooks(hooks_array: &mut Vec<Value>) {
    hooks_array.retain(|entry| {
        !entry
            .get("hooks")
            .and_then(|h| h.as_array())
            .map(|hooks| {
                hooks.iter().any(|hook| {
                    hook.get("command")
                        .and_then(|c| c.as_str())
                        .map(|cmd| cmd.contains("atm-hook"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false)
    });
}

/// Installs the atm-hook script to ~/.local/bin/
///
/// Creates the directory if it doesn't exist and sets executable permissions.
fn install_hook_script() -> Result<()> {
    let hook_path = hook_script_path();

    // Create parent directory if needed
    if let Some(parent) = hook_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    // Write the script
    fs::write(&hook_path, ATM_HOOK_SCRIPT)
        .with_context(|| format!("Failed to write {}", hook_path.display()))?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&hook_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&hook_path, perms)
            .with_context(|| format!("Failed to set permissions on {}", hook_path.display()))?;
    }

    Ok(())
}

/// Installs the ATM tmux keybindings file to ~/.config/atm/tmux-bindings.conf.
fn install_tmux_bindings() -> Result<()> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("could not determine config directory"))?
        .join("atm");

    std::fs::create_dir_all(&config_dir)?;

    let bindings_path = config_dir.join("tmux-bindings.conf");
    let content = r#"# ATM — Agent Tmux Manager bindings
# Source this in your .tmux.conf: source-file ~/.config/atm/tmux-bindings.conf

# Spawn a new Claude agent (default: below current pane)
bind C-n run-shell "atm spawn --target-pane #{pane_id}"

# Directional agent spawn (vim-style: h=left, j=below, k=above, l=right)
bind C-h run-shell "atm spawn --direction left --target-pane #{pane_id}"
bind C-j run-shell "atm spawn --direction below --target-pane #{pane_id}"
bind C-k run-shell "atm spawn --direction above --target-pane #{pane_id}"
bind C-l run-shell "atm spawn --direction right --target-pane #{pane_id}"

# Toggle ATM sidebar panel
bind C-a run-shell "atm toggle-panel"

# ATM popup overlay (alternative to sidebar)
bind C-s display-popup -E -w 35% -h 100% -x 0 "atm"

# Status bar integration (uncomment and add to status-right):
# set -g status-right '#(atm status) | %H:%M'
"#;

    std::fs::write(&bindings_path, content)?;
    println!("Installed tmux bindings: {}", bindings_path.display());
    println!(
        "Add to your .tmux.conf: source-file {}",
        bindings_path.display()
    );
    Ok(())
}

// ============================================================================
// Harness detection
// ============================================================================

/// True if Claude Code appears to be installed for this user.
///
/// We treat the existence of `~/.claude/` as authoritative — Claude
/// creates this directory on first run regardless of where its
/// binary lives.
fn detect_claude_code() -> bool {
    dirs::home_dir()
        .map(|h| h.join(".claude").exists())
        .unwrap_or(false)
}

/// True if pi appears to be installed for this user.
///
/// pi creates `~/.pi/agent/` on first run. Checking the directory
/// avoids depending on a particular install location for the binary
/// (npm global / nvm version / etc).
fn detect_pi() -> bool {
    dirs::home_dir()
        .map(|h| h.join(".pi/agent").exists())
        .unwrap_or(false)
}

// ============================================================================
// pi setup
// ============================================================================

/// Path under which we install the embedded `pi-atm` extension.
///
/// Pi resolves the `"packages/<name>"` settings entry relative to its
/// `agentDir` (`~/.pi/agent/`, not `~/.pi/`) — verified against
/// `package-manager.js`'s `globalBaseDir = this.agentDir` and
/// `agentDir = ~/.pi/agent`. So the install path is
/// `~/.pi/agent/packages/<name>/`.
fn pi_atm_package_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".pi/agent/packages/pi-atm"))
}

/// Path to pi's settings file.
fn pi_settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".pi").join("agent").join("settings.json"))
}

/// Reads pi's settings.json. Returns an empty object if not present.
fn read_pi_settings() -> Result<Value> {
    let path = pi_settings_path().context("Could not determine home directory")?;
    read_json_file_or_empty(&path)
}

fn write_pi_settings(settings: &Value) -> Result<()> {
    let path = pi_settings_path().context("Could not determine home directory")?;
    write_json_file_pretty(&path, settings)
}

/// Writes the embedded pi-atm extension files to
/// `~/.pi/packages/pi-atm/` (overwriting if present), then ensures
/// `"packages/pi-atm"` is in pi's settings `packages` array.
///
/// Returns `(files_written, settings_changed)` for caller's success
/// message.
fn install_pi_extension() -> Result<(bool, bool)> {
    let pkg_dir = pi_atm_package_dir().context("Could not determine home directory")?;
    let extensions_dir = pkg_dir.join("extensions");
    fs::create_dir_all(&extensions_dir)
        .with_context(|| format!("Failed to create {}", extensions_dir.display()))?;

    // Layout matches pi-amplike: package.json with pi.extensions
    // pointing at ./extensions/, and the .ts file inside.
    let ts_path = extensions_dir.join("pi-atm.ts");
    let pkg_path = pkg_dir.join("package.json");

    // Always write to refresh any in-place edits the user might have made.
    fs::write(&ts_path, PI_ATM_EXTENSION)
        .with_context(|| format!("Failed to write {}", ts_path.display()))?;
    fs::write(&pkg_path, PI_ATM_PACKAGE_JSON)
        .with_context(|| format!("Failed to write {}", pkg_path.display()))?;

    // Update pi's settings.json packages array.
    let mut settings = read_pi_settings()?;
    if settings.get("packages").is_none() {
        settings["packages"] = json!([]);
    }
    let packages = settings["packages"]
        .as_array_mut()
        .context("packages is not an array in pi settings.json")?;

    // Pi's local-package format: "packages/<name>" (relative to ~/.pi/).
    let entry = Value::String("packages/pi-atm".to_string());
    let already_present = packages.iter().any(|v| v == &entry);
    if !already_present {
        packages.push(entry);
        write_pi_settings(&settings)?;
        Ok((true, true))
    } else {
        Ok((true, false))
    }
}

/// Removes the pi-atm extension from `~/.pi/packages/` and from pi's
/// settings.json.
fn uninstall_pi_extension() -> Result<bool> {
    let mut changed = false;
    if let Some(pkg_dir) = pi_atm_package_dir() {
        if pkg_dir.exists() {
            fs::remove_dir_all(&pkg_dir)
                .with_context(|| format!("Failed to remove {}", pkg_dir.display()))?;
            changed = true;
        }
    }
    let mut settings = read_pi_settings()?;
    if let Some(packages) = settings.get_mut("packages").and_then(|p| p.as_array_mut()) {
        let before = packages.len();
        packages.retain(|v| v.as_str() != Some("packages/pi-atm"));
        if packages.len() < before {
            write_pi_settings(&settings)?;
            changed = true;
        }
    }
    Ok(changed)
}

/// Removes the atm-hook script from ~/.local/bin/
fn remove_hook_script() -> Result<bool> {
    let hook_path = hook_script_path();

    if hook_path.exists() {
        fs::remove_file(&hook_path)
            .with_context(|| format!("Failed to remove {}", hook_path.display()))?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Installs atm integration for every detected coding-agent harness.
pub fn setup() -> Result<()> {
    println!("Setting up ATM...\n");

    // Detect installed harnesses up-front so the user sees what
    // will (and won't) be configured.
    let claude = detect_claude_code();
    let pi = detect_pi();

    println!("Detected coding agents:");
    println!(
        "  {} Claude Code  (~/.claude/{})",
        if claude { "✓" } else { "✗" },
        if claude { "" } else { " not present" }
    );
    println!(
        "  {} pi           (~/.pi/agent/{})",
        if pi { "✓" } else { "✗" },
        if pi { "" } else { " not present" }
    );

    if !claude && !pi {
        println!("\nNo supported agent installations found. Install Claude Code or pi first.");
        return Ok(());
    }

    if claude {
        setup_claude_code()?;
    }

    if pi {
        setup_pi()?;
    }

    // Step N: Install tmux keybindings (vendor-neutral).
    println!();
    install_tmux_bindings()?;

    println!("\nNext step:");
    println!("  Run: atm");

    Ok(())
}

/// Wires `atm-hook` into Claude Code's `~/.claude/settings.json`.
fn setup_claude_code() -> Result<()> {
    println!("\nConfiguring Claude Code...");
    let hook_path = hook_script_path();
    print!("  Installing hook script to {}... ", hook_path.display());
    install_hook_script()?;
    println!("done");

    let mut settings = read_settings()?;

    // Ensure hooks object exists
    if settings.get("hooks").is_none() {
        settings["hooks"] = json!({});
    }

    let hooks = settings["hooks"]
        .as_object_mut()
        .context("hooks is not an object")?;

    let mut added = 0;

    for &hook_type in HOOK_TYPES {
        let hooks_array = hooks
            .entry(hook_type)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .context("hook type is not an array")?;

        if has_atm_hook(hooks_array) {
            println!("    {hook_type} - already configured");
        } else {
            hooks_array.push(create_hook_entry(hook_type));
            added += 1;
            println!("    {hook_type} - added");
        }
    }

    // statusLine
    let status_line_configured = if let Some(existing) = settings.get("statusLine") {
        if has_atm_status_line(existing) {
            println!("    statusLine - already configured");
            false
        } else {
            settings["statusLine"] = create_status_line_entry();
            println!("    statusLine - updated to use atm-hook");
            true
        }
    } else {
        settings["statusLine"] = create_status_line_entry();
        println!("    statusLine - added");
        true
    };

    if added > 0 || status_line_configured {
        write_settings(&settings)?;
        println!("  Claude Code configuration written.");
    } else {
        println!("  Claude Code already configured.");
    }
    Ok(())
}

/// Installs the `pi-atm` extension into `~/.pi/packages/pi-atm/` and
/// registers it in pi's settings.json `packages` array.
fn setup_pi() -> Result<()> {
    println!("\nConfiguring pi...");
    let (files_written, settings_changed) = install_pi_extension()?;
    if files_written {
        let pkg_dir = pi_atm_package_dir().unwrap_or_default();
        println!("    pi-atm.ts written to {}", pkg_dir.display());
    }
    if settings_changed {
        println!("    settings.json - added 'packages/pi-atm'");
    } else {
        println!("    settings.json - already references 'packages/pi-atm'");
    }
    println!("  pi configuration written.");
    Ok(())
}

/// Removes atm hooks from Claude Code settings and the hook script
pub fn uninstall() -> Result<()> {
    println!("Uninstalling ATM...\n");

    // Step 1: Remove from Claude Code settings
    println!("Removing Claude Code hooks...");
    let mut settings = read_settings()?;

    let mut removed = 0;
    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for &hook_type in HOOK_TYPES {
            if let Some(hooks_array) = hooks.get_mut(hook_type).and_then(|h| h.as_array_mut()) {
                let before = hooks_array.len();
                remove_atm_hooks(hooks_array);
                let after = hooks_array.len();

                if before != after {
                    removed += before - after;
                    println!("  {hook_type} - removed");
                }

                // Remove empty arrays
                if hooks_array.is_empty() {
                    hooks.remove(hook_type);
                }
            }
        }

        if removed > 0 {
            write_settings(&settings)?;
        }
    }

    if removed == 0 {
        println!("  No hooks found");
    }

    // Step 2: Remove statusLine if it uses atm-hook
    let mut status_line_removed = false;
    if let Some(status_line) = settings.get("statusLine") {
        if has_atm_status_line(status_line) {
            if let Some(obj) = settings.as_object_mut() {
                obj.remove("statusLine");
            }
            write_settings(&settings)?;
            println!("\nstatusLine configuration removed");
            status_line_removed = true;
        }
    }
    if !status_line_removed {
        println!("\nstatusLine - not configured by atm");
    }

    // Step 3: Remove the hook script
    let hook_path = hook_script_path();
    print!("\nRemoving hook script {}... ", hook_path.display());
    if remove_hook_script()? {
        println!("done");
    } else {
        println!("not found");
    }

    // Step 4: Uninstall pi-atm extension if present
    if detect_pi() {
        print!("\nRemoving pi-atm extension... ");
        match uninstall_pi_extension() {
            Ok(true) => println!("done"),
            Ok(false) => println!("not present"),
            Err(e) => println!("failed: {e}"),
        }
    }

    println!("\nATM uninstalled successfully!");
    Ok(())
}
