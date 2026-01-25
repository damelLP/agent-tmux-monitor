# Week 5+ Implementation Plan: Phases 4-5 (Rich UI + Advanced Features)

## Overview

This phase transforms the ATM TUI from a functional monitoring tool into a polished, production-ready application with a rich split-pane interface and optional advanced features. The focus is on delivering a professional user experience while maintaining the performance optimizations from Weeks 1-4.

**Timeline:** Days 1-7 (Phase 4 Required), Days 8-14+ (Phase 5 Optional)

**Status:** Weeks 1-4 must be complete before starting this phase.

## Prerequisites from Weeks 1-4

Before beginning Week 5+, verify the following are complete and tested:

### Week 1: Daemon Foundation
- [ ] Daemon process running with PID management
- [ ] Session data model and basic API
- [ ] Change detection system working (prevents unnecessary renders)
- [ ] Throttling mechanism tested (handles burst updates)
- [ ] Signal handling (graceful shutdown)

### Week 2: Core TUI
- [ ] Basic TUI renders session list
- [ ] Keyboard navigation working (j/k/Enter/q)
- [ ] Real-time updates via polling or IPC
- [ ] Session detail view functional

### Week 3: Integration
- [ ] Claude CLI hooks integrated (creates/updates sessions)
- [ ] Session lifecycle tracked (start/update/end)
- [ ] Cost calculation accurate
- [ ] Token tracking working

### Week 4: Testing & Refinement
- [ ] Unit tests passing for daemon and TUI
- [ ] Integration tests passing
- [ ] Performance benchmarks met
- [ ] Error handling robust

**If any of the above are incomplete, address them before proceeding to Week 5+.**

---

## Phase 4: Rich UI (Split-Pane Layout) - Days 1-7

**Goal:** Implement a professional split-pane interface with overview panel, session list, and detail panel. This phase is REQUIRED for production readiness.

### Day 1-2: Split-Pane Layout Architecture

#### Tasks
1. **Design Layout Structure**
   - Top panel: Overview/stats (15% height)
   - Middle panel: Session list (50% height)
   - Bottom panel: Detail view (35% height)
   - Responsive resizing based on terminal dimensions

2. **Implement Panel Container**
   ```rust
   // src/tui/layout.rs
   pub struct SplitPaneLayout {
       overview_height: u16,
       list_height: u16,
       detail_height: u16,
   }

   impl SplitPaneLayout {
       pub fn new(terminal_height: u16) -> Self {
           // Calculate panel heights based on terminal size
           // Minimum heights: overview=5, list=10, detail=8
       }

       pub fn resize(&mut self, new_height: u16) {
           // Recalculate panel heights
       }
   }
   ```

3. **Handle Terminal Resize Events**
   - Listen for `Event::Resize` in main event loop
   - Recalculate panel dimensions
   - Trigger full redraw

#### Testing
- [ ] Verify layout renders correctly at various terminal sizes
- [ ] Test resize events (expand/shrink terminal)
- [ ] Ensure minimum sizes respected (80x24 terminal minimum)

#### Success Criteria
- Clean split between three panels
- No overlap or rendering artifacts
- Smooth resize behavior

---

### Day 3-4: Overview Panel Implementation

#### Tasks
1. **Global Statistics Display**
   ```
   ┌─ Agent Tmux Monitor Monitor ─────────────────────────────────────────┐
   │ Active: 3 sessions | Total Today: 12 | Cost: $2.45         │
   │ Tokens (1h): 125K in / 32K out | Avg: 2.1K/min            │
   │ Context: Global window = 100K tokens                       │
   └─────────────────────────────────────────────────────────────┘
   ```

2. **Aggregate Metrics**
   - Total active sessions
   - Sessions completed today
   - Total cost (today/week/all-time)
   - Token throughput (last hour, moving average)
   - Global context window usage

3. **Real-Time Updates**
   - Refresh overview when sessions update
   - Use change detection to avoid unnecessary redraws
   - Apply throttling (max 2 updates/sec)

4. **Color Coding**
   - Green: Healthy metrics
   - Yellow: Warning thresholds (e.g., >$5/day)
   - Red: Critical thresholds (e.g., >$10/day)

#### Implementation Notes
- Leverage existing throttling from Week 1
- Cache aggregated stats, invalidate on session changes
- Use `ratatui::widgets::Paragraph` for text layout

#### Testing
- [ ] Verify all metrics display correctly
- [ ] Test with 0, 1, and many sessions
- [ ] Confirm color thresholds trigger appropriately
- [ ] Check update throttling (no flicker)

#### Success Criteria
- Overview panel provides at-a-glance status
- Updates in real-time without performance issues
- Clear visual hierarchy

---

### Day 5: Enhanced Session List Panel

#### Tasks
1. **Add Status Icons**
   ```
   ┌─ Sessions ──────────────────────────────────────────────────┐
   │ ● session-1234  [ACTIVE]   2.5K in/out  $0.05   5m ago     │
   │ ○ session-5678  [IDLE]     1.2K in/out  $0.02   15m ago    │
   │ ✓ session-9012  [COMPLETE] 5.0K in/out  $0.10   1h ago     │
   └─────────────────────────────────────────────────────────────┘
   ```
   - `●` Active (green)
   - `○` Idle (yellow)
   - `✓` Complete (gray)
   - `✗` Error (red)

2. **Column Formatting**
   - Session ID (truncated if needed)
   - Status badge
   - Token counts (formatted: 1.2K, 500, etc.)
   - Cost (formatted: $0.05)
   - Last activity (relative time: "5m ago", "2h ago")

3. **Selection Highlighting**
   - Highlight selected row
   - Clear visual distinction from other rows
   - Support keyboard navigation (j/k, arrow keys)

4. **Sorting Options**
   - Default: Most recent first
   - Support future sorting by cost, tokens, status

#### Implementation Notes
- Use `ratatui::widgets::List` or `Table` for layout
- Format numbers with `humanize` crate or custom formatter
- Use `chrono` for relative timestamps

#### Testing
- [ ] Icons render correctly in terminal
- [ ] Column alignment maintained with varying data
- [ ] Selection highlight visible
- [ ] Sorting works correctly

#### Success Criteria
- Session list is scannable and informative
- Visual feedback for selection
- Handles 0 to 100+ sessions gracefully

---

### Day 6: Detail Panel Implementation

#### Tasks
1. **Full Session Information Display**
   ```
   ┌─ Session Details ───────────────────────────────────────────┐
   │ ID: session-1234-5678-90ab                                  │
   │ Status: ACTIVE                                              │
   │ Started: 2026-01-23 14:30:22                               │
   │ Duration: 5m 32s                                           │
   │                                                            │
   │ Tokens:                                                    │
   │   Input:  2,543 tokens                                    │
   │   Output:   892 tokens                                    │
   │   Total:  3,435 tokens                                    │
   │                                                            │
   │ Cost: $0.052 (input: $0.038, output: $0.014)             │
   │                                                            │
   │ Model: claude-sonnet-4-5-20250929                         │
   │ Context: 200,000 token window                             │
   │                                                            │
   │ Working Directory: /home/user/project                     │
   │                                                            │
   │ Recent Activity:                                          │
   │   14:35:10 - Token update: +450 in, +120 out             │
   │   14:34:22 - Tool call: bash                              │
   │   14:33:45 - Token update: +320 in, +95 out              │
   └─────────────────────────────────────────────────────────────┘
   ```

2. **Information Hierarchy**
   - Primary info: ID, status, timing
   - Resource usage: Tokens, cost breakdown
   - Configuration: Model, context window
   - Activity log: Recent events (last 5-10)

3. **Dynamic Content**
   - Update when selected session changes
   - Show "No session selected" when list is empty
   - Scroll activity log if too long for panel

4. **Formatting**
   - Align labels and values
   - Use color for status indicators
   - Format large numbers with commas
   - Show cost with currency symbol

#### Implementation Notes
- Use `ratatui::widgets::Paragraph` with styled text
- Store activity events in session data (circular buffer, max 50)
- Use `textwrap` for long strings

#### Testing
- [ ] All fields display correctly
- [ ] Updates when selection changes
- [ ] Handles missing/incomplete data gracefully
- [ ] Scrolling works for long activity logs

#### Success Criteria
- Comprehensive view of session state
- Easy to read and understand
- Updates reflect real-time changes

---

### Day 7: Color Themes and Polish

#### Tasks
1. **Define Color Scheme**
   ```rust
   // src/tui/theme.rs
   pub struct Theme {
       pub active_fg: Color,
       pub idle_fg: Color,
       pub complete_fg: Color,
       pub error_fg: Color,
       pub border_fg: Color,
       pub selection_bg: Color,
       pub warning_fg: Color,
       pub critical_fg: Color,
   }

   impl Theme {
       pub fn default() -> Self {
           Self {
               active_fg: Color::Green,
               idle_fg: Color::Yellow,
               complete_fg: Color::Gray,
               error_fg: Color::Red,
               border_fg: Color::Cyan,
               selection_bg: Color::DarkGray,
               warning_fg: Color::Yellow,
               critical_fg: Color::Red,
           }
       }
   }
   ```

2. **Apply Consistent Colors**
   - Panel borders: Cyan
   - Active sessions: Green
   - Idle sessions: Yellow
   - Complete sessions: Gray
   - Errors: Red
   - Selection: Dark gray background
   - Cost warnings: Yellow (>$5), Red (>$10)

3. **Visual Polish**
   - Clean panel borders (Unicode box-drawing characters)
   - Proper padding (1 space inside borders)
   - Aligned text columns
   - Smooth scrolling

4. **Accessibility**
   - Ensure sufficient contrast
   - Don't rely solely on color (use icons too)
   - Test in different terminal emulators

#### Implementation Notes
- Use `ratatui::style::{Color, Style, Modifier}`
- Consider supporting custom themes in future (config file)
- Test on black/white/dark/light terminal backgrounds

#### Testing
- [ ] Colors render correctly in multiple terminals
- [ ] Contrast is readable
- [ ] Borders align properly
- [ ] No visual artifacts

#### Success Criteria
- Professional, polished appearance
- Consistent color usage throughout UI
- Accessible to colorblind users (icons + color)

---

### Phase 4 Completion Checklist

Before proceeding to Phase 5, verify:

- [ ] All three panels render correctly
- [ ] Overview shows accurate global stats
- [ ] Session list displays all active sessions with icons
- [ ] Detail panel shows complete session information
- [ ] Keyboard navigation works (j/k/Enter/q)
- [ ] Real-time updates work in all panels
- [ ] Color theme applied consistently
- [ ] Performance is smooth (no lag, flicker, or stuttering)
- [ ] Works in terminals 80x24 and larger
- [ ] Change detection prevents unnecessary redraws
- [ ] Throttling limits update frequency

**Phase 4 Deliverable:** A production-ready split-pane TUI with rich, real-time session monitoring.

---

## Phase 5: Advanced Features (OPTIONAL) - Days 8-14+

**Goal:** Add optional features to enhance usability and functionality. These are NOT required for production release and can be deferred or implemented incrementally based on user feedback.

**IMPORTANT:** Phase 5 represents approximately 40-50% additional scope beyond the core monitoring tool. Prioritize features based on user needs and avoid scope creep.

---

### Recommended Deferral: Features to Skip or Postpone

The following features add significant complexity and should be deferred unless explicitly required:

1. **Session History (SQLite)**
   - Rationale: Core tool monitors active/recent sessions. Persistent history is a "nice to have."
   - Alternative: Keep last N completed sessions in memory (circular buffer).
   - Defer until: User feedback indicates need for historical analysis.

2. **Session Control (Kill/Pause/Resume)**
   - Rationale: Requires complex process management and error handling.
   - Risk: Killing sessions mid-operation could corrupt state.
   - Defer until: Core monitoring is stable and tested.

3. **Export Features (JSON/CSV)**
   - Rationale: Export can be added later with minimal disruption.
   - Alternative: Users can inspect daemon state file directly.
   - Defer until: Users request data export for analysis.

4. **Configuration UI**
   - Rationale: Config files are sufficient for initial release.
   - Defer until: User requests for runtime configuration changes.

---

### Day 8-9: Filtering and Search (OPTIONAL)

**Priority:** Medium (useful but not critical)

#### Tasks
1. **Filter by Status**
   - Toggle filters: Show Active / Show Idle / Show Complete
   - Keybindings: `a` (active), `i` (idle), `c` (complete), `A` (all)

2. **Search by Session ID**
   - Press `/` to enter search mode
   - Type-ahead filtering of session list
   - Clear with `Esc`

3. **Filter UI Indicators**
   - Show active filters in header: `[Filters: Active, Idle]`
   - Display search term if searching: `[Search: "sess"]`

#### Implementation
```rust
// src/tui/filter.rs
pub struct SessionFilter {
    pub show_active: bool,
    pub show_idle: bool,
    pub show_complete: bool,
    pub search_term: Option<String>,
}

impl SessionFilter {
    pub fn matches(&self, session: &Session) -> bool {
        // Check status filter
        let status_match = match session.status {
            Status::Active => self.show_active,
            Status::Idle => self.show_idle,
            Status::Complete => self.show_complete,
        };

        // Check search term
        let search_match = self.search_term.as_ref()
            .map(|term| session.id.contains(term))
            .unwrap_or(true);

        status_match && search_match
    }
}
```

#### Testing
- [ ] Filters reduce session list correctly
- [ ] Search matches partial session IDs
- [ ] Filters persist across UI updates
- [ ] Clear filters restores full list

#### Defer If:
- Users don't request filtering in first release
- Session counts remain manageable (<20 active)

---

### Day 10-11: Configuration System (OPTIONAL)

**Priority:** Low (hard-coded defaults are sufficient initially)

#### Tasks
1. **Create Config File Format**
   ```toml
   # ~/.config/atm/config.toml
   [ui]
   theme = "default"
   refresh_rate_ms = 500
   max_sessions_displayed = 100

   [thresholds]
   cost_warning = 5.0
   cost_critical = 10.0

   [daemon]
   state_file = "~/.local/share/atm/state.json"
   log_file = "~/.local/share/atm/daemon.log"
   ```

2. **Config Loading**
   - Load on TUI/daemon startup
   - Fall back to defaults if file missing
   - Validate config values

3. **Apply Config**
   - Use configured refresh rate
   - Apply threshold values
   - Honor file paths

#### Implementation
```rust
// src/config.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub ui: UiConfig,
    pub thresholds: ThresholdConfig,
    pub daemon: DaemonConfig,
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = dirs::config_dir()
            .ok_or(ConfigError::NoConfigDir)?
            .join("atm/config.toml");

        if config_path.exists() {
            let contents = std::fs::read_to_string(config_path)?;
            let config: Config = toml::from_str(&contents)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }
}
```

#### Testing
- [ ] Config loads from file if present
- [ ] Defaults used if file missing
- [ ] Invalid config values caught and logged
- [ ] Config changes take effect on restart

#### Defer If:
- Default values work for all users
- No requests for customization

---

### Day 12: Help Screen (OPTIONAL)

**Priority:** Medium (useful for new users)

#### Tasks
1. **Create Help Modal**
   ```
   ┌─ Help ──────────────────────────────────────────────────────┐
   │ Navigation:                                                 │
   │   j / ↓       - Move down                                  │
   │   k / ↑       - Move up                                    │
   │   Enter       - View session details                       │
   │   q / Esc     - Quit                                       │
   │                                                            │
   │ Filtering: (if implemented)                                │
   │   a           - Toggle active sessions                     │
   │   i           - Toggle idle sessions                       │
   │   c           - Toggle complete sessions                   │
   │   /           - Search sessions                            │
   │                                                            │
   │ Other:                                                     │
   │   ?           - Show this help                             │
   │   r           - Force refresh                              │
   │                                                            │
   │ Press any key to close                                     │
   └─────────────────────────────────────────────────────────────┘
   ```

2. **Toggle Help**
   - Press `?` to show help overlay
   - Press any key to dismiss

3. **Help Content**
   - List all keybindings
   - Brief description of each panel
   - Link to full documentation

#### Implementation
- Render help as overlay on top of main UI
- Center in terminal
- Use modal style (dim background)

#### Testing
- [ ] Help displays correctly
- [ ] All keybindings documented
- [ ] Dismisses on any key press

#### Defer If:
- User base is small and can reference README
- Keybindings are intuitive enough

---

### Day 13: Performance Optimization (OPTIONAL)

**Priority:** High IF performance issues arise, Low otherwise

#### Tasks
1. **Profiling**
   - Use `cargo flamegraph` to identify hotspots
   - Measure render time per frame
   - Check memory usage with many sessions

2. **Optimizations**
   - Cache rendered widgets between frames
   - Use dirty flags to skip unchanged panels
   - Optimize string formatting (avoid allocations)
   - Batch session updates (coalesce rapid changes)

3. **Benchmarking**
   - Target: <16ms per frame (60 FPS)
   - Test with 100+ sessions
   - Verify no memory leaks over 24hr run

#### Implementation Notes
- Leverage change detection from Week 1
- Use `Rc<RefCell<>>` or `Arc<Mutex<>>` for shared state (avoid cloning)
- Profile before optimizing (avoid premature optimization)

#### Testing
- [ ] No noticeable lag with 100 sessions
- [ ] Memory usage stable over time
- [ ] Frame time consistently <16ms

#### Defer If:
- Current performance is acceptable
- Session counts remain low in practice

---

### Day 14+: Documentation and Polish (OPTIONAL)

**Priority:** Medium (important for open-source release)

#### Tasks
1. **User Documentation**
   - README with installation instructions
   - Usage guide with screenshots/GIFs
   - Configuration reference
   - Troubleshooting section

2. **Developer Documentation**
   - Architecture overview
   - Code comments and rustdoc
   - Contributing guidelines
   - Testing instructions

3. **Packaging**
   - Create release builds (optimized, stripped)
   - Cargo package metadata (Cargo.toml)
   - Consider `cargo install` support
   - Consider distribution packages (brew, apt, etc.)

4. **Final Polish**
   - Fix any outstanding bugs
   - Clean up debug logging
   - Remove dead code
   - Run `clippy` and fix warnings

#### Testing
- [ ] Documentation is clear and accurate
- [ ] Release build works on clean system
- [ ] All tests pass in release mode

#### Defer If:
- Initial release is for personal use only
- Documentation can be added iteratively

---

## Scope Management: Avoiding Feature Creep

**Critical Reminder:** The Agent Tmux Monitor project started as a simple monitoring tool. Each additional feature adds:
- Development time (1-3 days per feature)
- Testing complexity (integration tests, edge cases)
- Maintenance burden (bugs, updates, user support)
- Cognitive load (more code to understand)

### Recommended Minimum Viable Product (MVP)

For production release, prioritize ONLY:
1. Phase 4: Rich UI (split-pane layout) - REQUIRED
2. Help screen (Day 12) - RECOMMENDED
3. Basic filtering (Day 8) - OPTIONAL

### Features to Defer Until Post-Launch

Based on the critique identifying 40-50% scope creep, defer these features until user feedback justifies them:

- Session history (SQLite database)
- Session control (kill/pause/resume)
- Export features (JSON/CSV)
- Configuration UI (runtime changes)
- Advanced filtering (regex, multiple criteria)
- Custom themes (beyond default)
- Remote monitoring (network access)
- Plugins/extensions

### Decision Framework

Before implementing any Phase 5 feature, ask:
1. Does a real user need this RIGHT NOW?
2. Can this be added later without breaking changes?
3. Is the development time justified by the benefit?
4. Does this complicate the core use case?

If the answer to #2 is "yes" (can be added later), DEFER IT.

---

## Production Readiness Checklist

Before declaring Agent Tmux Monitor "production-ready," verify:

### Functionality
- [ ] Daemon starts reliably and survives errors
- [ ] TUI displays all active sessions correctly
- [ ] Session lifecycle tracked accurately (start/update/end)
- [ ] Cost calculations match expected values
- [ ] Real-time updates work without lag
- [ ] Keyboard navigation is intuitive and responsive

### Performance
- [ ] Handles 50+ active sessions without slowdown
- [ ] Memory usage stable over 24+ hours
- [ ] Frame rate stays above 30 FPS
- [ ] No unnecessary CPU usage when idle

### Reliability
- [ ] No crashes during normal operation
- [ ] Graceful handling of daemon restart
- [ ] Recovers from corrupted state file
- [ ] Error messages are clear and actionable

### User Experience
- [ ] UI is visually clean and professional
- [ ] Color scheme is readable on common terminals
- [ ] Help documentation is clear
- [ ] Installation process is simple

### Code Quality
- [ ] All tests passing (unit + integration)
- [ ] No `clippy` warnings
- [ ] Code formatted with `rustfmt`
- [ ] Critical paths have error handling

### Documentation
- [ ] README covers installation and usage
- [ ] Key components have rustdoc comments
- [ ] Known issues documented
- [ ] Contribution guidelines present (if open-source)

---

## Success Criteria for Week 5+

### Phase 4 Success (Days 1-7)
- Split-pane UI is polished and professional
- Overview panel provides actionable insights
- Session list is scannable and informative
- Detail panel shows comprehensive session state
- User feedback: "This looks and feels production-ready"

### Phase 5 Success (Days 8-14+)
- Only features with clear user value are implemented
- No feature creep beyond original vision
- Each feature is tested and documented
- Tool remains simple and focused on monitoring

### Overall Success
- Agent Tmux Monitor is a reliable, performant monitoring tool
- Users can confidently track Claude usage and costs
- Codebase is maintainable and well-tested
- Future features can be added without refactoring

---

## Next Steps After Week 5+

Once Phases 4-5 are complete, consider:

1. **Beta Testing**
   - Share with small group of users
   - Gather feedback on UI and features
   - Identify bugs and usability issues

2. **Iterative Improvement**
   - Prioritize features based on user feedback
   - Fix reported bugs promptly
   - Resist adding features without clear demand

3. **Community Building** (if open-source)
   - Create GitHub repository with clear README
   - Set up issue tracker for bug reports
   - Welcome contributions aligned with project vision

4. **Long-Term Maintenance**
   - Keep dependencies updated
   - Monitor for security issues
   - Maintain compatibility with Claude CLI updates

---

## Conclusion

Week 5+ transforms Agent Tmux Monitor from a functional tool to a polished, production-ready application. Phase 4 (Rich UI) is essential and should be completed fully. Phase 5 (Advanced Features) should be approached selectively, implementing only features with clear user value and deferring the rest to avoid scope creep.

Remember: **A simple tool that works perfectly is infinitely more valuable than a complex tool that does everything poorly.** Stay focused on the core mission: reliable, real-time monitoring of Claude sessions with cost and token tracking.

Good luck with the implementation!
