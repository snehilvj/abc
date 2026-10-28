//! Drawing help: every key for the focused pane, then the leader's commands
//! and the keys that work everywhere. The keys come from the keymap, so help
//! shows what they do now, rebound or not.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::Span;

use super::dialog::{self, Dialog, Row};
use crate::app::App;
use crate::keys::{self, Action, Context, Keymap, Keys};
use crate::theme::Theme;

/// The box's width.
const WIDTH: u16 = 64;
/// Cells for the keys before what they do.
const KEYS: usize = 18;

/// Actions whose keys show side by side, as bound in the pane, or in another
/// context: visual mode's copy keys, say.
struct Group(Option<Context>, &'static [Action]);

/// A line of help: its groups of keys, and what they do.
type Line = (&'static [Group], &'static str);

const fn here(actions: &'static [Action]) -> Group {
    Group(None, actions)
}

use Action::*;

const SIDEBAR: &[Line] = &[
    (&[here(&[Down, Up])], "move between worktrees"),
    (&[here(&[First, Last])], "first · last"),
    (&[here(&[Open])], "open worktree"),
];

const EXPLORER: &[Line] = &[
    (&[here(&[Down, Up])], "move"),
    (&[here(&[First, Last])], "first · last"),
    (&[here(&[Open])], "open file · expand or collapse folder"),
    (&[here(&[CopyReference])], "copy @path for Claude Code"),
];

const VIEWER: &[Line] = &[
    (
        &[here(&[Left, Down, Up, Right])],
        "move; a count first repeats",
    ),
    (
        &[
            here(&[WordForward, WordBackward, WordEnd]),
            here(&[BigWordForward, BigWordBackward, BigWordEnd]),
        ],
        "words · WORDS",
    ),
    (
        &[here(&[LineStart, FirstNonBlank, LineEnd])],
        "line start · first non-blank · end",
    ),
    (
        &[here(&[First, Last])],
        "top · end; with a count, that line",
    ),
    (
        &[here(&[ScreenTop, ScreenMiddle, ScreenBottom])],
        "top · middle · bottom of the screen",
    ),
    (
        &[
            here(&[ParagraphBackward, ParagraphForward]),
            here(&[MatchingBracket]),
        ],
        "paragraph · matching bracket",
    ),
    (
        &[
            here(&[FindForward, TillForward, FindBackward, TillBackward]),
            here(&[RepeatFind, RepeatFindBack]),
        ],
        "to a character · again",
    ),
    (
        &[here(&[HalfPageDown, HalfPageUp])],
        "half a page down · up",
    ),
    (&[here(&[PageDown, PageUp])], "a page down · up"),
    (
        &[here(&[ScrollDown, ScrollUp]), here(&[CursorLineMiddle])],
        "scroll a line · centre the cursor",
    ),
    (
        &[
            here(&[Search, NextMatch, PreviousMatch]),
            here(&[SearchWord, SearchWordBack]),
        ],
        "find · next · this word",
    ),
    (&[here(&[GoToLine])], "go to a line"),
    (
        &[
            here(&[VisualChars, VisualLines]),
            Group(Some(Context::Visual), &[Copy, CopyLine]),
        ],
        "select · copy · copy lines",
    ),
    (
        &[here(&[CopyReference])],
        "copy @path:lines for Claude Code",
    ),
    (&[here(&[JumpBack])], "back from a jump"),
    (&[here(&[FocusLeft])], "back out a step; then the explorer"),
];

/// Global keys of a single key, where the pane leaves them working.
const EVERYWHERE: &[Line] = &[
    (&[here(&[Switcher])], "switcher"),
    (&[here(&[LastWorktree])], "last worktree"),
    (
        &[here(&[FocusLeft, FocusRight])],
        "move focus between panes",
    ),
    (&[here(&[Help])], "this help"),
    (&[here(&[Quit])], "quit (abcd keeps running)"),
];

pub fn draw(buf: &mut Buffer, area: Rect, app: &App, theme: &Theme) {
    let map = &app.keymap;
    let (pane, name, lines) = match app.context() {
        Context::Explorer => (Context::Explorer, "Explorer", EXPLORER),
        Context::Viewer | Context::Visual => (Context::Viewer, "Viewer", VIEWER),
        Context::Sidebar | Context::Global => (Context::Sidebar, "Sidebar", SIDEBAR),
    };
    let heading = |text: &'static str| {
        Row::new(Span::styled(
            text,
            theme.accent().add_modifier(Modifier::BOLD),
        ))
    };
    let row = |keys: String, what: String| {
        Row::new(vec![
            Span::styled(format!("  {keys:<KEYS$} "), theme.bold()),
            Span::styled(what, theme.text()),
        ])
    };

    let mut body = vec![Row::default(), heading(name)];
    for (groups, what) in lines {
        let keys = line_keys(map, pane, groups, |_| true);
        if !keys.is_empty() {
            body.push(row(keys, what.to_string()));
        }
    }
    body.push(Row::default());
    body.push(heading("Leader"));
    for (keys, action) in map.global_bindings(pane) {
        if keys.0.len() > 1 {
            body.push(row(keys.to_string(), action.describe()));
        }
    }
    body.push(Row::default());
    body.push(heading("Everywhere"));
    // The leader's sequences are listed above.
    let single = |keys: &Keys| keys.0.len() == 1;
    let global = map.global_bindings(pane);
    for (groups, what) in EVERYWHERE {
        let keys = line_keys(map, pane, groups, |k| {
            single(k) && global.iter().any(|(g, _)| *g == k)
        });
        if !keys.is_empty() {
            body.push(row(keys, what.to_string()));
        }
    }
    body.push(Row::default());
    body.push(Row::new(Span::styled(
        "Change any key in [keys] in config.toml.",
        theme.dim(),
    )));

    let help = map
        .keys_for(Context::Global, Help)
        .map(keys::short)
        .unwrap_or_default();
    let footer = vec![(format!("esc or {help}"), "close")];
    let title = format!("Keys · {name}");
    // The body, a rule, the footer and the box's two edges.
    let height = body.len() as u16 + 4;
    let width = WIDTH.min(area.width.saturating_sub(4));
    let x = area.x + (area.width - width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let dialog = Dialog {
        title: &title,
        header: Vec::new(),
        body,
        footer,
    };
    dialog::draw(buf, (x, y), width, dialog, theme);
}

/// A line's keys, as bound in `pane`, keeping those `keep` allows: the first
/// key of each action, side by side, then each action's second, and so on
/// while they fit. "j k  ↓ ↑"; "w b e  W B E" for two groups.
fn line_keys(
    map: &Keymap,
    pane: Context,
    groups: &[Group],
    keep: impl Fn(&Keys) -> bool,
) -> String {
    // Per group, per level: the keys of that rank.
    let levels: Vec<Vec<String>> = groups
        .iter()
        .map(|Group(context, actions)| {
            let keys: Vec<Vec<String>> = actions
                .iter()
                .map(|a| {
                    let keys = map.all_keys_for(context.unwrap_or(pane), *a);
                    keys.into_iter()
                        .filter(|k| keep(k))
                        .map(|k| k.to_string())
                        .collect()
                })
                .collect();
            let depth = keys.iter().map(Vec::len).max().unwrap_or(0);
            (0..depth)
                .map(|n| {
                    let rank: Vec<&str> = keys
                        .iter()
                        .filter_map(|k| k.get(n))
                        .map(String::as_str)
                        .collect();
                    rank.join(" ")
                })
                .collect()
        })
        .collect();
    let deepest = levels.iter().map(Vec::len).max().unwrap_or(0);
    let mut text = String::new();
    for depth in (1..=deepest).rev() {
        text = levels
            .iter()
            .filter(|l| !l.is_empty())
            .map(|l| l[..depth.min(l.len())].join("  "))
            .collect::<Vec<_>>()
            .join("  ");
        if text.chars().count() <= KEYS {
            break;
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(pane: Context, groups: &[Group]) -> String {
        line_keys(&Keymap::defaults(), pane, groups, |_| true)
    }

    #[test]
    fn a_lines_keys_come_from_the_keymap_and_fit_their_column() {
        assert_eq!(keys(Context::Explorer, &[here(&[Down, Up])]), "j k  ↓ ↑");
        assert_eq!(
            keys(Context::Viewer, VIEWER[0].0),
            "h j k l  ← ↓ ↑ →",
            "every key, in ranks"
        );
        assert_eq!(keys(Context::Viewer, VIEWER[1].0), "w b e  W B E");
        assert_eq!(
            keys(Context::Viewer, &[here(&[PageDown, PageUp])]),
            "ctrl-f ctrl-b",
            "pgdn pgup would overflow"
        );
        assert_eq!(keys(Context::Viewer, VIEWER[12].0), "v V  y Y");
        assert_eq!(keys(Context::Viewer, &[here(&[ScreenTop])]), "H");
    }

    #[test]
    fn a_key_rebound_shows_in_help() {
        let mut map = Keymap::defaults();
        map.bind(Context::Explorer, "x", Open).unwrap();
        let open = line_keys(&map, Context::Explorer, EXPLORER[2].0, |_| true);
        assert_eq!(open, "enter  o  x");
    }
}
