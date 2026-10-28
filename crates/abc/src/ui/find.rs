//! Drawing find in files: the query and how many lines matched, then the
//! results grouped by file, in a dialog over the faded screen where the
//! switcher sits.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};

use super::dialog::{self, Dialog, Row};
use super::label::{self, blank, no_icon};
use super::viewer::thousands;
use crate::app::{App, plural};
use crate::find::{self, Progress, Row as Found};
use crate::icons;
use crate::search::Search;
use crate::theme::Theme;
use crate::viewer::shown;

/// The box's width, and how far down the screen it sits: the switcher's.
const WIDTH: u16 = 96;
const TOP: u16 = 3;
/// Cells for line numbers.
const NUMBER: usize = 5;

pub fn draw(buf: &mut Buffer, area: Rect, app: &App, theme: &Theme) {
    let f = &app.find;
    let width = WIDTH.min(area.width.saturating_sub(4));
    let x = area.x + (area.width - width) / 2;
    let inner = width.saturating_sub(4) as usize;
    let name = f
        .worktree
        .as_deref()
        .and_then(|p| app.find(p))
        .map_or("", |(_, w)| w.name.as_str());
    let title = format!("Find in {name}");

    let rows = f.rows();
    let selected = f.selected_row();
    // Keep the selection in sight, with its file's row above it when there's room.
    let mut offset = f.offset.get();
    if let Some(sel) = selected {
        if sel < offset {
            let header = sel > 0 && matches!(rows[sel - 1], Found::File(_));
            offset = if header { sel - 1 } else { sel };
        } else if sel >= offset + find::ROWS {
            offset = sel + 1 - find::ROWS;
        }
    }
    f.offset.set(offset);

    let search = Search::new(f.query.clone(), true);
    let mut body: Vec<Row> = message(app, name, theme).map_or_else(Vec::new, |m| vec![Row::new(m)]);
    let room = inner.saturating_sub(label::icon_width(theme) + NUMBER + 2);
    for (i, row) in rows.iter().enumerate().skip(offset) {
        if body.len() == find::ROWS {
            break;
        }
        body.push(match *row {
            Found::File(n) => {
                let file = &f.files[n];
                let (icon, color) = icons::file(&file.path, theme);
                Row::new(vec![
                    Span::styled(icon.to_string(), theme.style(color)),
                    blank(theme.icon_gap),
                    Span::styled(file.path.as_str(), theme.bold()),
                ])
                .right(Span::styled(thousands(file.lines.len()), theme.dim()))
            }
            Found::Line(n, l) => {
                let line = &f.files[n].lines[l];
                let mut spans = vec![
                    no_icon(theme),
                    Span::styled(format!("{:>NUMBER$}", line.number), theme.dim()),
                    blank(2),
                ];
                spans.extend(marked(&line.text, &search, room, theme));
                Row::new(spans).selected(Some(i) == selected)
            }
        });
    }
    // The box keeps its size as results come and go.
    while body.len() < find::ROWS {
        body.push(Row::default());
    }
    let footer = vec![
        ("enter".into(), "open"),
        ("↑↓".into(), "move"),
        ("ctrl-u".into(), "clear"),
        ("esc".into(), "close"),
    ];
    let header = vec![dialog::query(&f.input, count(&app.find), inner, theme)];
    let dialog = Dialog {
        title: &title,
        header,
        body,
        footer,
    };
    dialog::draw(buf, (x, area.y + TOP), width, dialog, theme);
}

/// "25 lines · 5 files", "1,000+ lines · 1 file", or "searching…".
fn count(f: &find::Find) -> String {
    match f.progress {
        Progress::Idle | Progress::Failed(_) => String::new(),
        Progress::Searching => "searching…".into(),
        Progress::Done { capped } => {
            let lines = f.lines();
            let word = if lines == 1 { "line" } else { "lines" };
            let more = if capped { "+" } else { "" };
            format!(
                "{}{more} {word} · {}",
                thousands(lines),
                plural(f.files.len(), "file")
            )
        }
    }
}

/// What to say instead of results.
fn message<'a>(app: &App, name: &str, theme: &Theme) -> Option<Line<'a>> {
    let f = &app.find;
    let spans = match &f.progress {
        Progress::Idle if f.query.is_empty() => vec![Span::styled(
            format!("Type to find text in {name}'s files."),
            theme.dim(),
        )],
        Progress::Done { .. } if f.files.is_empty() => vec![Span::styled(
            format!("No lines match \"{}\".", f.query),
            theme.dim(),
        )],
        Progress::Failed(why) => label::badge(
            icons::FAILURE,
            format!("Can't search: {why}"),
            theme.error,
            theme,
        ),
        _ => return None,
    };
    Some(Line::from(spans))
}

/// A matching line as shown: without its indentation, control characters as
/// their symbols, every match bold in the accent colour. A line too long to
/// show its first match is cut before it.
fn marked<'a>(text: &str, search: &Search, room: usize, theme: &Theme) -> Vec<Span<'a>> {
    let line = shown(text);
    let chars: Vec<char> = line.trim_start().chars().collect();
    let trimmed: String = chars.iter().collect();
    let matches = search.matches(&trimmed);
    let start = match matches.first() {
        Some(&(first, end)) if end > room => first.saturating_sub(10),
        _ => 0,
    };
    let mut spans: Vec<Span> = Vec::new();
    let mut room = room;
    if start > 0 {
        spans.push(Span::styled("…", theme.dim()));
        room -= 1;
    }
    let cut = chars.len() - start > room;
    let end = if cut {
        start + room.saturating_sub(1)
    } else {
        chars.len()
    };
    let hit = theme.accent().add_modifier(Modifier::BOLD);
    for (i, c) in chars.iter().enumerate().take(end).skip(start) {
        let style = if matches.iter().any(|&(a, b)| a <= i && i < b) {
            hit
        } else {
            theme.text()
        };
        match spans.last_mut() {
            Some(last) if last.style == style && last.content != "…" => {
                last.content.to_mut().push(*c)
            }
            _ => spans.push(Span::styled(c.to_string(), style)),
        }
    }
    if cut {
        spans.push(Span::styled("…", theme.dim()));
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(spans: &[Span]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn a_line_loses_its_indentation_and_shows_its_match() {
        let theme = Theme::dark();
        let search = Search::new("gstin", true);
        let spans = marked("\t  lookup(gstin: string)", &search, 40, &theme);
        assert_eq!(text(&spans), "lookup(gstin: string)");
        assert_eq!(spans[1].content, "gstin");
        let long = format!("{}gstin end", "x".repeat(60));
        let spans = marked(&long, &search, 30, &theme);
        assert_eq!(text(&spans), format!("…{}gstin end", "x".repeat(10)));
        let escape = marked("a \u{1b}[31m gstin", &search, 40, &theme);
        assert_eq!(
            text(&escape),
            "a \u{241b}[31m gstin",
            "control characters as symbols"
        );
    }
}
