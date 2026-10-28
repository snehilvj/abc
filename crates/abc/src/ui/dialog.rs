//! Dialogs: a box over the faded screen with a title, header rows, the body
//! and a footer of keys, each part ruled off from the next. The switcher and
//! the repository dialogs share it.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use tui_input::Input;

use crate::theme::Theme;

/// A line in a dialog: text from the left, text against the right, maybe
/// selected, maybe with a text cursor.
#[derive(Debug, Default)]
pub struct Row<'a> {
    pub left: Line<'a>,
    pub right: Line<'a>,
    pub selected: bool,
    /// Cells from the left where a text cursor sits.
    pub cursor: Option<usize>,
}

impl<'a> Row<'a> {
    pub fn new(left: impl Into<Line<'a>>) -> Self {
        Self {
            left: left.into(),
            ..Self::default()
        }
    }

    pub fn right(self, right: impl Into<Line<'a>>) -> Self {
        Self {
            right: right.into(),
            ..self
        }
    }

    pub fn selected(self, selected: bool) -> Self {
        Self { selected, ..self }
    }

    pub fn cursor(self, cursor: usize) -> Self {
        Self {
            cursor: Some(cursor),
            ..self
        }
    }
}

pub struct Dialog<'a> {
    pub title: &'a str,
    pub header: Vec<Row<'a>>,
    pub body: Vec<Row<'a>>,
    /// Keys and what they do, as the key bar shows them.
    pub footer: Vec<(String, &'a str)>,
}

/// A query being typed: "> " and the text with its cursor, scrolled to keep
/// the cursor in sight, and `count` on the right.
pub fn query<'a>(input: &'a Input, count: String, inner: usize, theme: &Theme) -> Row<'a> {
    // "> ", the text, the cursor's cell, a blank, then the count.
    let room = inner.saturating_sub(2 + 1 + 1 + count.chars().count());
    let scroll = input.visual_scroll(room);
    let mut skipped = 0;
    let shown: String = input
        .value()
        .chars()
        .skip_while(|c| {
            let skip = skipped < scroll;
            if skip {
                skipped += crate::viewer::width(&[*c]);
            }
            skip
        })
        .collect();
    Row::new(vec![
        Span::styled("> ", theme.accent()),
        Span::styled(shown, theme.bold()),
    ])
    .right(Span::styled(count, theme.dim()))
    .cursor(2 + input.visual_cursor() - scroll)
}

/// Keys and what they do, the key bar's way: "enter open · esc close".
pub fn key_hints<'a>(items: Vec<(String, &'a str)>, theme: &Theme) -> Line<'a> {
    let mut spans = Vec::new();
    for (i, (keys, label)) in items.into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" · ", theme.dim()));
        }
        spans.push(Span::styled(keys, theme.bold()));
        spans.push(Span::styled(format!(" {label}"), theme.text()));
    }
    Line::from(spans)
}

/// Draws `dialog` `width` columns wide with its top left corner at `x`, `y`,
/// and returns the box it took.
pub fn draw(
    buf: &mut Buffer,
    (x, y): (u16, u16),
    width: u16,
    dialog: Dialog,
    theme: &Theme,
) -> Rect {
    enum Part<'a> {
        Row(Row<'a>),
        Rule,
        Footer(Line<'a>),
    }
    let mut parts: Vec<Part> = Vec::new();
    if !dialog.header.is_empty() {
        parts.extend(dialog.header.into_iter().map(Part::Row));
        parts.push(Part::Rule);
    }
    parts.extend(dialog.body.into_iter().map(Part::Row));
    parts.push(Part::Rule);
    parts.push(Part::Footer(key_hints(dialog.footer, theme)));

    let area = Rect::new(x, y, width, parts.len() as u16 + 2).intersection(buf.area);
    let (x1, y1) = (area.right() - 1, area.bottom() - 1);
    let border = theme.text();
    for row in area.top()..area.bottom() {
        for col in area.left()..area.right() {
            buf[(col, row)].reset();
        }
    }
    for col in area.left()..=x1 {
        buf[(col, y)].set_symbol("─").set_style(border);
        buf[(col, y1)].set_symbol("─").set_style(border);
    }
    for row in y..=y1 {
        buf[(x, row)].set_symbol("│").set_style(border);
        buf[(x1, row)].set_symbol("│").set_style(border);
    }
    for (col, row, corner) in [(x, y, "┌"), (x1, y, "┐"), (x, y1, "└"), (x1, y1, "┘")] {
        buf[(col, row)].set_symbol(corner).set_style(border);
    }
    let title = format!(" {} ", dialog.title);
    buf.set_string(x + 1, y, title, theme.bold());

    let inner = width.saturating_sub(4);
    for (i, part) in parts.into_iter().enumerate() {
        let row = y + 1 + i as u16;
        if row >= y1 {
            break;
        }
        match part {
            Part::Rule => {
                for col in x..=x1 {
                    buf[(col, row)].set_symbol("─").set_style(border);
                }
                buf[(x, row)].set_symbol("├");
                buf[(x1, row)].set_symbol("┤");
            }
            Part::Footer(line) => {
                buf.set_line(x + 2, row, &line, inner);
            }
            Part::Row(r) => {
                if r.selected {
                    let line = Rect::new(x + 1, row, width.saturating_sub(2), 1);
                    buf.set_style(line, Style::new().bg(theme.selection));
                }
                let right = r.right.width() as u16;
                if right > 0 {
                    buf.set_line(x1 - 1 - right, row, &r.right, right);
                }
                let room = inner.saturating_sub(if right > 0 { right + 1 } else { 0 });
                buf.set_line(x + 2, row, &r.left, room);
                if let Some(cursor) = r.cursor
                    && (cursor as u16) < inner
                {
                    let cell = &mut buf[(x + 2 + cursor as u16, row)];
                    cell.set_style(Style::new().add_modifier(Modifier::REVERSED));
                }
            }
        }
    }
    area
}
