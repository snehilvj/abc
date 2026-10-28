//! Drawing the viewer: a header with the file's icon, path and language; a
//! gutter of line numbers; and only the lines on screen, coloured from their
//! highlight spans once those exist.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::label::{badge, labelled};
use crate::highlight::{self, Kind, Lang};
use crate::icons;
use crate::theme::Theme;
use crate::viewer::{Document, Viewer, width};

pub fn kind_style(kind: Kind, theme: &Theme) -> Style {
    let s = &theme.syntax;
    match kind {
        Kind::Keyword => theme.style(s.keyword),
        Kind::String => theme.style(s.string),
        Kind::Comment => theme.style(s.comment).add_modifier(Modifier::ITALIC),
        Kind::Type => theme.style(s.r#type),
        Kind::Function => theme.style(s.function),
        Kind::Constant => theme.style(s.constant),
        Kind::Property => theme.style(s.property),
        Kind::Punctuation => theme.style(s.punctuation),
    }
}

/// The header: icon, path (trimmed from the left to fit), language.
pub fn header<'a>(viewer: &Viewer, width: usize, focused: bool, theme: &Theme) -> Line<'a> {
    let name = viewer.name();
    let lang = match &viewer.document {
        Document::Binary { .. } => "binary",
        _ => Lang::of(name).label(),
    };
    let (icon, color) = icons::file(name, theme);
    let path = viewer.path.join("/");
    let tail = format!(" · {lang} ");
    // " " + icon + gap + path + tail
    let room = width.saturating_sub(1 + 1 + theme.icon_gap + tail.chars().count());
    let path = super::ltrunc(&path, room);
    let mut spans = vec![Span::raw(" ")];
    spans.extend(labelled(
        icon.to_string(),
        theme.style(color),
        path,
        theme.header(focused),
        theme,
    ));
    spans.push(Span::styled(tail, theme.dim()));
    Line::from(spans)
}

/// The text, or a notice in its place, in `area`. Focused, it shows the cursor.
pub fn draw(buf: &mut Buffer, area: Rect, viewer: Option<&Viewer>, focused: bool, theme: &Theme) {
    let notice = |buf: &mut Buffer, lines: Vec<Vec<Span>>| {
        for (i, spans) in lines.into_iter().enumerate() {
            buf.set_line(
                area.x + 1,
                area.y + i as u16,
                &Line::from(spans),
                area.width.saturating_sub(1),
            );
        }
    };
    let Some(viewer) = viewer else {
        return notice(
            buf,
            vec![
                vec![Span::styled("No file open.", theme.text())],
                vec![],
                vec![Span::styled("Pick one in the explorer, or", theme.dim())],
                vec![Span::styled("press space f to search files.", theme.dim())],
            ],
        );
    };
    viewer.height.set(area.height as usize);
    match &viewer.document {
        Document::Loading => notice(
            buf,
            vec![labelled(
                icons::SYNC,
                theme.dim(),
                format!("Loading {}…", viewer.name()),
                theme.dim(),
                theme,
            )],
        ),
        Document::Failed(why) => notice(
            buf,
            vec![
                badge(
                    icons::FAILURE,
                    format!("Can't read {}", viewer.name()),
                    theme.error,
                    theme,
                ),
                vec![],
                vec![Span::styled(why.as_str(), theme.text())],
            ],
        ),
        Document::Binary { size } => notice(
            buf,
            vec![labelled(
                icons::INFO,
                theme.dim(),
                format!("Binary file · {}", bytes(*size)),
                theme.dim(),
                theme,
            )],
        ),
        Document::TooLarge { size, limit } => notice(
            buf,
            vec![
                badge(
                    icons::WARNING,
                    format!("Too large to show · {}", bytes(*size)),
                    theme.warning,
                    theme,
                ),
                vec![],
                vec![Span::styled(
                    format!("The limit is {}.", bytes(*limit)),
                    theme.dim(),
                )],
            ],
        ),
        Document::Text(text) => {
            let total = text.lines.len();
            let gutter = total.to_string().len().max(2);
            let numbers = 1 + gutter + 2;
            let x = area.x + numbers as u16;
            // One column spare on the right.
            let room = (area.width as usize).saturating_sub(numbers + 1);
            let cursor = viewer.cursor;
            let left = viewer.left(room);
            for (row, index) in (viewer.top()..total).take(area.height as usize).enumerate() {
                let y = area.y + row as u16;
                let current = focused && index == cursor.line;
                let number = format!(" {:>gutter$}  ", index + 1);
                let number_style = if current { theme.text() } else { theme.dim() };
                buf.set_string(area.x, y, &number, number_style);
                if current {
                    let line = Rect::new(x, y, room as u16, 1);
                    buf.set_style(line, Style::new().bg(theme.cursor_line));
                }
                let spans = text
                    .spans
                    .as_ref()
                    .map(|s| s[index].as_slice())
                    .unwrap_or(&[]);
                let line = skip(styled(&text.lines[index], spans, theme), left);
                if line.width() > room {
                    buf.set_line(x, y, &line, room.saturating_sub(1) as u16);
                    buf.set_string(x + room.saturating_sub(1) as u16, y, "…", theme.dim());
                } else {
                    buf.set_line(x, y, &line, room as u16);
                }
                let chars: Vec<char> = text.lines[index].chars().collect();
                if let Some((from, to, by_line)) = viewer.selected().filter(|_| focused)
                    && (from.line..=to.line).contains(&index)
                {
                    let start = if by_line || index > from.line {
                        0
                    } else {
                        from.col
                    };
                    // An empty line shows one selected cell, as in Vim.
                    let end = if by_line || index < to.line {
                        chars.len().max(1)
                    } else {
                        to.col + 1
                    };
                    let mut cx = 0;
                    for col in left..end {
                        let w = chars.get(col).map_or(1, |c| width(&[*c]));
                        if col >= start {
                            for dx in cx..(cx + w).min(room) {
                                buf[(x + dx as u16, y)].set_bg(theme.selection);
                            }
                        }
                        cx += w;
                        if cx >= room {
                            break;
                        }
                    }
                }
                if current {
                    let col = cursor.col.min(chars.len());
                    let before = width(&chars[left.min(col)..col]);
                    if before < room {
                        let cell = &mut buf[(x + before as u16, y)];
                        cell.set_style(Style::new().add_modifier(Modifier::REVERSED));
                    }
                }
            }
        }
    }
}

/// `line` without its first `n` characters, for a view scrolled sideways.
fn skip(line: Line<'_>, mut n: usize) -> Line<'_> {
    if n == 0 {
        return line;
    }
    let mut spans = Vec::new();
    for span in line.spans {
        let count = span.content.chars().count();
        if n >= count {
            n -= count;
            continue;
        }
        let rest: String = span.content.chars().skip(n).collect();
        n = 0;
        spans.push(Span::styled(rest, span.style));
    }
    Line::from(spans)
}

/// One line, split into the default colour and its highlight spans.
fn styled<'a>(line: &'a str, spans: &[highlight::Span], theme: &Theme) -> Line<'a> {
    let mut out = Vec::new();
    let mut at = 0;
    for span in spans {
        if span.start > at {
            out.push(Span::styled(&line[at..span.start], theme.text()));
        }
        if span.end > span.start.max(at) {
            out.push(Span::styled(
                &line[span.start.max(at)..span.end],
                kind_style(span.kind, theme),
            ));
        }
        at = at.max(span.end);
    }
    if at < line.len() {
        out.push(Span::styled(&line[at..], theme.text()));
    }
    Line::from(out)
}

/// The lines shown, as the separator under the viewer says: "1–34 of 58".
pub fn range(viewer: &Viewer) -> Option<String> {
    let Document::Text(text) = &viewer.document else {
        return None;
    };
    let total = text.lines.len();
    let first = viewer.top() + 1;
    let last = (first - 1 + viewer.height.get()).min(total);
    Some(format!(
        "{}–{} of {}",
        thousands(first),
        thousands(last),
        thousands(total)
    ))
}

/// Where the cursor is, as "line:column": "12:5".
pub fn position(viewer: &Viewer) -> Option<String> {
    viewer.has_text().then(|| {
        format!(
            "{}:{}",
            thousands(viewer.cursor.line + 1),
            viewer.cursor.col + 1
        )
    })
}

fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A size in KB or MB, one decimal.
pub fn bytes(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1} MB", n as f64 / 1_000_000.0),
        n if n >= 1_000 => format!("{:.1} KB", n as f64 / 1_000.0),
        n => format!("{n} bytes"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_counts_read_naturally() {
        assert_eq!(bytes(24_123), "24.1 KB");
        assert_eq!(bytes(12_400_000), "12.4 MB");
        assert_eq!(bytes(512), "512 bytes");
        assert_eq!(thousands(9967), "9,967");
        assert_eq!(thousands(10000), "10,000");
        assert_eq!(thousands(58), "58");
    }

    #[test]
    fn a_line_scrolled_sideways_loses_its_first_characters() {
        let line = Line::from(vec![Span::raw("fn"), Span::raw(" main"), Span::raw("()")]);
        let parts =
            |l: Line| -> Vec<String> { l.spans.iter().map(|s| s.content.to_string()).collect() };
        assert_eq!(parts(skip(line.clone(), 4)), ["ain", "()"]);
        assert_eq!(parts(skip(line.clone(), 2)), [" main", "()"]);
        assert_eq!(parts(skip(line, 0)), ["fn", " main", "()"]);
    }

    #[test]
    fn a_line_splits_into_plain_and_highlighted_parts() {
        let spans = [
            highlight::Span {
                start: 0,
                end: 2,
                kind: Kind::Keyword,
            },
            highlight::Span {
                start: 3,
                end: 7,
                kind: Kind::Function,
            },
        ];
        let line = styled("fn main() {}", &spans, &Theme::dark());
        let parts: Vec<&str> = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(parts, ["fn", " ", "main", "() {}"]);
    }
}
