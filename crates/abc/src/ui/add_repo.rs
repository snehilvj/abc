//! Drawing the add repository dialog: where from, the name, and a line
//! saying what abcd made of them and what adding will do.

use abc_core::model::RepoCheck;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
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

    let path_hint = "a folder on this machine, or a git URL to clone";
    let path = field(
        "Path",
        &add.path,
        path_hint,
        add.field == Field::Path,
        inner,
        theme,
    );

    let default = add.default_name();
    let mut name = field(
        "Name",
        &add.name,
        default,
        add.field == Field::Name,
        inner,
        theme,
    );
    if add.name.value().is_empty() {
        let note = match (default.is_empty(), add.is_url()) {
            (true, _) => "(the folder's or the repository's)",
            (false, true) => "(the repository's name)",
            (false, false) => "(the folder's name)",
        };
        let used = label::width(&name.left.spans);
        name.left
            .spans
            .push(blank((LABEL + NAME).saturating_sub(used).max(1)));
        name.left.spans.push(Span::styled(note, theme.dim()));
    }

    let body = vec![
        Row::default(),
        path,
        Row::default(),
        name,
        Row::default(),
        Row::new(check_line(app, add, theme)),
    ];
    let footer = vec![
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

/// "Path  …" or "Name  …": the label in the accent colour while the field
/// has focus, the text scrolled to keep its cursor in sight. Empty, it shows
/// `stand_in`: a hint for the path, the default for the name.
fn field<'a>(
    label: &'a str,
    input: &'a Input,
    stand_in: &'a str,
    focused: bool,
    inner: usize,
    theme: &Theme,
) -> Row<'a> {
    let label_style = if focused {
        theme.accent().add_modifier(Modifier::BOLD)
    } else {
        theme.dim()
    };
    let mut spans = vec![Span::styled(format!("{label:<LABEL$}"), label_style)];
    let scroll = input.visual_scroll(inner.saturating_sub(LABEL + 1));
    if input.value().is_empty() {
        // The path's hint sits after its cursor; the name's default stands where it will be.
        let hint = label == "Path";
        let style = if hint { theme.dim() } else { theme.text() };
        let text = if hint && focused {
            format!(" {stand_in}")
        } else {
            stand_in.to_string()
        };
        spans.push(Span::styled(text, style));
    } else {
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
        spans.push(Span::styled(shown, theme.text()));
    }
    let row = Row::new(spans);
    if focused {
        row.cursor(LABEL + input.visual_cursor() - scroll)
    } else {
        row
    }
}

/// What abcd made of the path and name, or how adding is going.
fn check_line<'a>(app: &App, add: &AddRepo, theme: &Theme) -> Line<'a> {
    let failure = |text: String| label::badge(icons::failure(), text, theme.error, theme);
    let success = |text: String| label::badge(icons::success(), text, theme.ok, theme);
    let progress = |text: String| labelled(icons::sync(), theme.dim(), text, theme.dim(), theme);
    let name = add.name().to_string();
    // A repository abcd can add still needs a free name.
    let named = |ok: String| {
        if name.is_empty() {
            failure("type a name".into())
        } else if app.name_taken(&name) {
            failure(format!("a repository is already named {name}"))
        } else {
            success(ok)
        }
    };
    let spans: Vec<Span> = match (&add.stage, &add.check) {
        (Stage::Adding, _) => match add.clone_into() {
            Some(into) => progress(format!("cloning into {into}")),
            None => progress("adding".into()),
        },
        (Stage::Failed(why), _) => failure(format!("abcd: {why}")),
        (_, Check::Idle) => Vec::new(),
        (_, Check::Checking) => progress("checking".into()),
        (_, Check::Checked(Err(why))) => failure(format!("abcd: {why}")),
        (_, Check::Checked(Ok(check))) => match check {
            RepoCheck::Ready { worktrees } => named(format!(
                "git repository · {} found",
                plural(*worktrees, "worktree")
            )),
            RepoCheck::Remote { into } => named(format!("clones into {into}")),
            RepoCheck::NotFound if add.is_url() => failure("no repository at that URL".into()),
            RepoCheck::NotFound => failure("no such folder".into()),
            RepoCheck::NotGit => failure("not a git repository".into()),
            RepoCheck::AlreadyAdded { name } => failure(format!("already added as {name}")),
            RepoCheck::TargetExists { into } => {
                failure(format!("{into} already exists · name it something else"))
            }
        },
    };
    Line::from(spans)
}
