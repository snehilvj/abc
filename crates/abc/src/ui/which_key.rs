//! Drawing the which-key popup: what can follow an unfinished key sequence,
//! in a box above the key bar at the bottom right. It doesn't fade the screen.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::Span;

use super::dialog::{self, Dialog, Row};
use crate::app::App;
use crate::keys::{Keys, Next};
use crate::theme::Theme;

/// The box's width.
const WIDTH: u16 = 38;
/// Cells for each key before what it does.
const KEY: usize = 3;

pub fn draw(buf: &mut Buffer, area: Rect, app: &App, theme: &Theme) {
    let next = app.keymap.next(app.context(), &app.pending);
    if next.is_empty() {
        return;
    }
    let body: Vec<Row> = next
        .into_iter()
        .map(|(key, then)| {
            let (what, style) = match then {
                Next::Action(action) => (action.describe(), theme.text()),
                Next::Group(name) => (format!("+{name}"), theme.accent()),
            };
            Row::new(vec![
                Span::styled(format!("{:<KEY$}", key.to_string()), theme.bold()),
                Span::styled(what, style),
            ])
        })
        .collect();
    let title = Keys(app.pending.clone()).to_string();
    let footer = vec![("esc".into(), "cancel"), ("bksp".into(), "back")];
    // Two cells in from the frame's right edge; its bottom just above the rule over the key bar.
    let width = WIDTH.min(area.width.saturating_sub(4));
    let height = body.len() as u16 + 4;
    let x = area.right().saturating_sub(2 + width);
    let y = area.bottom().saturating_sub(3 + height);
    let dialog = Dialog {
        title: &title,
        header: Vec::new(),
        body,
        footer,
    };
    dialog::draw(buf, (x, y), width, dialog, theme);
    // The keys typed so far, in the accent colour, as the title.
    buf.set_string(
        x + 2,
        y,
        &title,
        theme.accent().add_modifier(Modifier::BOLD),
    );
}
