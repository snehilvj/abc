//! Drawing the remove repository dialog: which repository, that its files
//! stay, and how removing it is going.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use super::dialog::{self, Dialog, Row};
use super::label;
use crate::app::{App, Removal, RemoveRepo};
use crate::icons;
use crate::theme::Theme;

/// The dialog's width.
const WIDTH: u16 = 60;

pub fn draw(buf: &mut Buffer, area: Rect, app: &App, remove: &RemoveRepo, theme: &Theme) {
    let width = WIDTH.min(area.width.saturating_sub(4));
    let x = area.x + (area.width - width) / 2;
    let missing = app
        .repos
        .iter()
        .any(|r| r.name == remove.name && r.is_missing());
    let note = if missing {
        "Its folder is gone already; only the registration goes."
    } else {
        "Files stay on disk; only the registration goes."
    };
    let progress = match &remove.stage {
        Removal::Asking => Vec::new(),
        Removal::Removing => {
            label::labelled(icons::SYNC, theme.dim(), "removing", theme.dim(), theme)
        }
        Removal::Failed(why) => {
            label::badge(icons::FAILURE, format!("abcd: {why}"), theme.error, theme)
        }
    };
    let body = vec![
        Row::default(),
        Row::new(vec![
            Span::styled("Remove ", theme.text()),
            Span::styled(remove.name.as_str(), theme.bold()),
            Span::styled(" from abc?", theme.text()),
        ]),
        Row::default(),
        Row::new(Span::styled(note, theme.dim())),
        Row::new(Line::from(progress)),
    ];
    let footer = match remove.stage {
        Removal::Asking => vec![("y".into(), "remove"), ("n".into(), "keep")],
        Removal::Removing => Vec::new(),
        Removal::Failed(_) => vec![("esc".into(), "close")],
    };
    let dialog = Dialog {
        title: "Remove repository",
        header: Vec::new(),
        body,
        footer,
    };
    // The body, a rule, the footer and the box's two edges.
    let height = 5 + 2 + 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    dialog::draw(buf, (x, y), width, dialog, theme);
}
