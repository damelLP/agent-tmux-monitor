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
///
/// # Arguments
/// * `frame` - The frame to render into
/// * `area` - The full terminal area (popup will be centered within it)
pub fn render_help_popup(frame: &mut Frame, area: Rect) {
    let popup_area = centered_popup(60, 70, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let key_style = Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD);
    let heading_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let mut lines = vec![Line::from("")];
    let mut current_category = None;

    for entry in KEYBINDING_HINTS {
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

    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(popup, popup_area);
}
