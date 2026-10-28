//! Drawing the add repository dialog: the path with its completions, the
//! name, and a line saying what abcd made of the path.

use abc_core::model::RepoCheck;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use tui_input::Input;

use super::dialog::{self, Dialog, Row};
use super::label::{self, blank, labelled};
use crate::add_repo::{AddRepo, Check, Field, Stage};
use crate::app::{App, plural};
use crate::icons;
use crate::theme::Theme;

/// The dialog's width, and how far down the screen it sits.
const WIDTH: u16 = 72;
const TOP: u16 = 12;
/// "Path  " and "Name  ".
const LABEL: usize = 6;
/// Cells the name takes before its note.
const NAME: usize = 24;

pub fn draw(buf: &mut Buffer, area: Rect, app: &App, add: &AddRepo, theme: &Theme) {
    let width = WIDTH.min(area.width.saturating_sub(4));
    let x = area.x + (area.width - width) / 2;
    let inner = width.saturating_sub(4) as usize;

    let mut body = vec![Row::default()];
    body.push(field(
        "Path",
        &add.path,
        None,
        add.field == Field::Path,
        inner,
        theme,
    ));
    if let Some(c) = &add.completions {
        for (i, dir) in c.dirs.iter().enumerate() {
            let mut spans = vec![blank(LABEL)];
            spans.extend(labelled(
                icons::FOLDER,
                theme.style(theme.folder),
                dir.as_str(),
                theme.text(),
                theme,
            ));
            body.push(Row::new(spans).selected(c.selected == Some(i)));
        }
    }
    body.push(Row::default());
    // An empty name means the folder's, shown once abcd has found a repository there.
    let found = matches!(add.check, Check::Checked(Ok(RepoCheck::Ready { .. })));
    let default = add
        .name
        .value()
        .is_empty()
        .then(|| if found { add.folder_name() } else { "" });
    let mut name = field(
        "Name",
        &add.name,
        default,
        add.field == Field::Name,
        inner,
        theme,
    );
    if let Some(folder) = default {
        let note = if folder.is_empty() {
            "(the folder name)"
        } else {
            "(defaults to the folder name)"
        };
        let used = label::width(&name.left.spans);
        name.left
            .spans
            .push(blank((LABEL + NAME).saturating_sub(used).max(1)));
        name.left.spans.push(Span::styled(note, theme.dim()));
    }
    body.push(name);
    body.push(Row::default());
    body.push(Row::new(check_line(app, add, theme)));

    let footer = vec![
        ("tab".into(), "complete"),
        ("↑↓".into(), "path/name"),
        ("enter".into(), "add"),
        ("esc".into(), "cancel"),
    ];
    let dialog = Dialog {
        title: "Add repository",
        header: Vec::new(),
        body,
        footer,
    };
    dialog::draw(buf, (x, area.y + TOP), width, dialog, theme);
}

/// "Path  ~/code/…" or "Name  …": the label in the accent colour while the
/// field has focus, the text scrolled to keep its cursor in sight. A name
/// left empty shows the folder's in its place.
fn field<'a>(
    label: &'a str,
    input: &'a Input,
    default: Option<&'a str>,
    focused: bool,
    inner: usize,
    theme: &Theme,
) -> Row<'a> {
    let label_style = if focused {
        theme.accent().add_modifier(Modifier::BOLD)
    } else {
        theme.dim()
    };
    let room = inner.saturating_sub(LABEL + 1);
    let scroll = input.visual_scroll(room);
    let mut skipped = 0;
    let text: String = match default {
        Some(name) => name.to_string(),
        None => input
            .value()
            .chars()
            .skip_while(|c| {
                let skip = skipped < scroll;
                if skip {
                    skipped += crate::viewer::width(&[*c]);
                }
                skip
            })
            .collect(),
    };
    let row = Row::new(vec![
        Span::styled(format!("{label:<LABEL$}"), label_style),
        Span::styled(text, theme.text()),
    ]);
    if focused {
        row.cursor(LABEL + input.visual_cursor() - scroll)
    } else {
        row
    }
}

/// What abcd made of the path, a taken name, or how adding is going.
fn check_line<'a>(app: &App, add: &AddRepo, theme: &Theme) -> Line<'a> {
    let failure = |text: String| label::badge(icons::FAILURE, text, theme.error, theme);
    let progress = |text: &'a str| labelled(icons::SYNC, theme.dim(), text, theme.dim(), theme);
    let spans: Vec<Span> = match (&add.stage, &add.check) {
        (Stage::Adding, _) => progress("adding"),
        (Stage::Failed(why), _) => failure(format!("abcd: {why}")),
        (_, Check::Idle) => Vec::new(),
        (_, Check::Checking) => progress("checking"),
        (_, Check::Checked(Err(why))) => failure(format!("abcd: {why}")),
        (_, Check::Checked(Ok(check))) => match check {
            RepoCheck::Ready { .. } if app.name_taken(add.name()) => {
                failure(format!("the name {} is taken", add.name()))
            }
            RepoCheck::Ready { .. } if add.name().is_empty() => failure("type a name".into()),
            RepoCheck::Ready { worktrees } => label::badge(
                icons::SUCCESS,
                format!("git repository · {} found", plural(*worktrees, "worktree")),
                theme.ok,
                theme,
            ),
            RepoCheck::NotFound => failure("no such folder".into()),
            RepoCheck::NotGit => failure("not a git repository".into()),
            RepoCheck::AlreadyAdded { name } => failure(format!("already added as {name}")),
        },
    };
    Line::from(spans).style(Style::default())
}
