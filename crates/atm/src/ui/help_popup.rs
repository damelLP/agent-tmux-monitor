//! Help popup overlay showing all keybindings.
//!
//! Renders a centered popup on top of the existing layout, listing
//! available keyboard shortcuts grouped by category. Content is derived
//! from [`KEYBINDING_HINTS`] in `keybinding.rs`.

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::keybinding::{HintCategory, KEYBINDING_HINTS};

use super::layout::centered_popup;

/// Renders the help popup overlay.
///
/// Clears the background behind the popup and renders a bordered
/// paragraph listing all keybindings grouped by category.
/// Filters out tmux-only entries when not running inside tmux.
///
/// # Arguments
/// * `frame` - The frame to render into
/// * `area` - The full terminal area (popup will be centered within it)
pub fn render_help_popup(frame: &mut Frame, area: Rect) {
    let popup_area = centered_popup(60, 70, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let in_tmux = crate::tmux::is_in_tmux();
    let lines = build_help_lines(in_tmux);

    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(popup, popup_area);
}

/// Builds the styled content lines for the help popup.
///
/// Groups keybindings by category with headings, and filters out
/// tmux-only entries when `in_tmux` is false.
fn build_help_lines(in_tmux: bool) -> Vec<Line<'static>> {
    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let heading_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let mut lines = vec![Line::from("")];
    let mut current_category = None;

    for entry in KEYBINDING_HINTS {
        // Skip tmux-only entries when not in tmux
        if entry.tmux_only && !in_tmux {
            continue;
        }

        // Insert category heading when category changes
        if current_category != Some(entry.category) {
            if current_category.is_some() {
                lines.push(Line::from(""));
            }
            let heading = match entry.category {
                HintCategory::Navigation => "  Navigation",
                HintCategory::Actions => "  Actions",
            };
            lines.push(Line::from(Span::styled(heading, heading_style)));
            lines.push(Line::from(""));
            current_category = Some(entry.category);
        }

        lines.push(Line::from(vec![
            Span::styled(format!("    {:<11} ", entry.help_key), key_style),
            Span::raw(entry.help_desc),
        ]));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    /// Extract the raw text content from a Line (stripping styles).
    fn line_text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    // -- build_help_lines tests -----------------------------------------------

    #[test]
    fn test_lines_start_with_blank() {
        let lines = build_help_lines(true);
        assert!(!lines.is_empty());
        assert_eq!(line_text(&lines[0]), "");
    }

    #[test]
    fn test_navigation_heading_present() {
        let lines = build_help_lines(true);
        let texts: Vec<String> = lines.iter().map(line_text).collect();
        assert!(
            texts.iter().any(|t| t.contains("Navigation")),
            "Expected Navigation heading in lines: {texts:?}"
        );
    }

    #[test]
    fn test_actions_heading_present() {
        let lines = build_help_lines(true);
        let texts: Vec<String> = lines.iter().map(line_text).collect();
        assert!(
            texts.iter().any(|t| t.contains("Actions")),
            "Expected Actions heading in lines: {texts:?}"
        );
    }

    #[test]
    fn test_categories_separated_by_blank_line() {
        let lines = build_help_lines(true);
        let texts: Vec<String> = lines.iter().map(line_text).collect();

        // Find the Actions heading — it should be preceded by a blank line
        // (separating it from the Navigation category).
        let actions_idx = texts
            .iter()
            .position(|t| t.contains("Actions"))
            .expect("Actions heading should exist");

        assert!(actions_idx >= 2, "Actions heading too early to have separator");
        assert_eq!(
            texts[actions_idx - 1], "",
            "Expected blank line before Actions heading"
        );
    }

    #[test]
    fn test_entry_format_key_and_description() {
        let lines = build_help_lines(true);
        let texts: Vec<String> = lines.iter().map(line_text).collect();

        // The first keybinding entry is "j / ↓" → "Move down"
        let first_entry = texts
            .iter()
            .find(|t| t.contains("Move down"))
            .expect("Expected 'Move down' entry");

        // Check the key is left-padded and formatted
        assert!(
            first_entry.starts_with("    "),
            "Entry should be indented: {first_entry:?}"
        );
        assert!(
            first_entry.contains("Move down"),
            "Entry should contain description"
        );
    }

    #[test]
    fn test_all_non_tmux_hints_present_when_in_tmux() {
        let lines = build_help_lines(true);
        let texts: Vec<String> = lines.iter().map(line_text).collect();

        // Every hint should appear (including tmux-only ones)
        for entry in KEYBINDING_HINTS {
            assert!(
                texts.iter().any(|t| t.contains(entry.help_desc)),
                "Missing entry: {:?}",
                entry.help_desc
            );
        }
    }

    #[test]
    fn test_tmux_only_entries_filtered_when_not_in_tmux() {
        let lines_with_tmux = build_help_lines(true);
        let lines_without_tmux = build_help_lines(false);

        // There should be fewer lines when tmux-only entries are filtered
        assert!(
            lines_without_tmux.len() < lines_with_tmux.len(),
            "Expected fewer lines without tmux ({} vs {})",
            lines_without_tmux.len(),
            lines_with_tmux.len()
        );

        // The tmux-only entry ("Jump to session") should be absent
        let texts: Vec<String> = lines_without_tmux.iter().map(line_text).collect();
        let tmux_entries: Vec<_> = KEYBINDING_HINTS
            .iter()
            .filter(|e| e.tmux_only)
            .collect();

        for entry in &tmux_entries {
            assert!(
                !texts.iter().any(|t| t.contains(entry.help_desc)),
                "tmux-only entry should be filtered: {:?}",
                entry.help_desc
            );
        }
    }

    #[test]
    fn test_tmux_only_entries_present_when_in_tmux() {
        let lines = build_help_lines(true);
        let texts: Vec<String> = lines.iter().map(line_text).collect();

        let tmux_entries: Vec<_> = KEYBINDING_HINTS
            .iter()
            .filter(|e| e.tmux_only)
            .collect();

        for entry in &tmux_entries {
            assert!(
                texts.iter().any(|t| t.contains(entry.help_desc)),
                "tmux-only entry should be present when in tmux: {:?}",
                entry.help_desc
            );
        }
    }

    #[test]
    fn test_key_column_width_consistent() {
        let lines = build_help_lines(true);

        // Entries (not headings/blanks) should have exactly 2 spans:
        // styled key + raw description
        for line in &lines {
            if line.spans.len() == 2 {
                let key_span = &line.spans[0].content;
                // Key column should be "    {:<11} " = 16 chars total.
                // Use chars().count() because some keys contain multi-byte
                // Unicode (e.g. ↓, ↑ are 3 bytes each).
                let char_count = key_span.chars().count();
                assert_eq!(
                    char_count, 16,
                    "Key column width should be 16 chars, got {char_count} for {key_span:?}",
                );
            }
        }
    }

    // -- render smoke tests ---------------------------------------------------

    #[test]
    fn test_render_help_popup_80x24() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_help_popup(frame, frame.area());
            })
            .unwrap();
    }

    #[test]
    fn test_render_help_popup_40x12() {
        let backend = TestBackend::new(40, 12);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_help_popup(frame, frame.area());
            })
            .unwrap();
    }

    #[test]
    fn test_render_help_popup_minimal_size() {
        let backend = TestBackend::new(10, 5);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_help_popup(frame, frame.area());
            })
            .unwrap();
    }

    #[test]
    fn test_render_help_popup_wide_terminal() {
        let backend = TestBackend::new(200, 50);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|frame| {
                render_help_popup(frame, frame.area());
            })
            .unwrap();
    }
}
