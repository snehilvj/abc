//! Key sequences to actions, Vim style, with a `space` leader.
//!
//! crokey parses and normalises each key. Sequences ("space r a", "g g") are
//! matched here: a key that starts a longer binding waits for the next one.

use std::fmt;
use std::str::FromStr;

use crokey::KeyCombination;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    Quit,
    Help,
    Switcher,
    FindFile,
    FindWorktree,
    LastWorktree,
    AddRepo,
    RemoveRepo,
    FocusLeft,
    FocusRight,
    Down,
    Up,
    First,
    Last,
    Open,
    HalfPageDown,
    HalfPageUp,
    PageDown,
    PageUp,
    /// Try starting abcd again after it failed to start.
    Retry,
    // The viewer's cursor, Vim style.
    Left,
    Right,
    WordForward,
    WordBackward,
    WordEnd,
    BigWordForward,
    BigWordBackward,
    BigWordEnd,
    LineStart,
    FirstNonBlank,
    LineEnd,
    ScreenTop,
    ScreenMiddle,
    ScreenBottom,
    ScrollDown,
    ScrollUp,
    CursorLineTop,
    CursorLineMiddle,
    CursorLineBottom,
    ParagraphForward,
    ParagraphBackward,
    MatchingBracket,
    FindForward,
    FindBackward,
    TillForward,
    TillBackward,
    RepeatFind,
    RepeatFindBack,
    JumpBack,
}

impl FromStr for Action {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        Ok(match s {
            "quit" => Self::Quit,
            "help" => Self::Help,
            "switcher" => Self::Switcher,
            "find_file" => Self::FindFile,
            "find_worktree" => Self::FindWorktree,
            "last_worktree" => Self::LastWorktree,
            "add_repo" => Self::AddRepo,
            "remove_repo" => Self::RemoveRepo,
            "focus_left" => Self::FocusLeft,
            "focus_right" => Self::FocusRight,
            "down" => Self::Down,
            "up" => Self::Up,
            "first" | "top" => Self::First,
            "last" | "bottom" => Self::Last,
            "open" => Self::Open,
            "half_page_down" => Self::HalfPageDown,
            "half_page_up" => Self::HalfPageUp,
            "page_down" => Self::PageDown,
            "page_up" => Self::PageUp,
            "retry" => Self::Retry,
            "left" => Self::Left,
            "right" => Self::Right,
            "word_forward" => Self::WordForward,
            "word_backward" => Self::WordBackward,
            "word_end" => Self::WordEnd,
            "big_word_forward" => Self::BigWordForward,
            "big_word_backward" => Self::BigWordBackward,
            "big_word_end" => Self::BigWordEnd,
            "line_start" => Self::LineStart,
            "first_non_blank" => Self::FirstNonBlank,
            "line_end" => Self::LineEnd,
            "screen_top" => Self::ScreenTop,
            "screen_middle" => Self::ScreenMiddle,
            "screen_bottom" => Self::ScreenBottom,
            "scroll_down" => Self::ScrollDown,
            "scroll_up" => Self::ScrollUp,
            "cursor_line_top" => Self::CursorLineTop,
            "cursor_line_middle" => Self::CursorLineMiddle,
            "cursor_line_bottom" => Self::CursorLineBottom,
            "paragraph_forward" => Self::ParagraphForward,
            "paragraph_backward" => Self::ParagraphBackward,
            "matching_bracket" => Self::MatchingBracket,
            "find_forward" => Self::FindForward,
            "find_backward" => Self::FindBackward,
            "till_forward" => Self::TillForward,
            "till_backward" => Self::TillBackward,
            "repeat_find" => Self::RepeatFind,
            "repeat_find_back" => Self::RepeatFindBack,
            "jump_back" => Self::JumpBack,
            other => return Err(format!("unknown action {other:?}")),
        })
    }
}

/// Where a binding applies. Pane bindings win over global ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Context {
    Global,
    Sidebar,
    Explorer,
    Viewer,
}

/// A sequence of normalised keys.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Keys(pub Vec<KeyCombination>);

impl FromStr for Keys {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let keys = s
            .split_whitespace()
            .map(|k| {
                crokey::parse(k)
                    .map(normalise)
                    .map_err(|e| format!("{s:?}: {e}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if keys.is_empty() {
            return Err("an empty key sequence".into());
        }
        Ok(Self(keys))
    }
}

impl fmt::Display for Keys {
    /// "gg", "G", "space r a", "ctrl-p": letters run together, anything else is spaced.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let labels: Vec<String> = self.0.iter().map(|k| label(*k)).collect();
        let plain = self.0.iter().all(|k| {
            k.as_letter().is_some_and(|c| c.is_ascii_alphabetic()) || is_shifted_letter(*k)
        });
        f.write_str(&labels.join(if plain { "" } else { " " }))
    }
}

/// The key as a key-bar label: `^p` instead of `ctrl-p`.
pub fn short(keys: &Keys) -> String {
    keys.to_string().replace("ctrl-", "^")
}

fn is_shifted_letter(k: KeyCombination) -> bool {
    k.modifiers == KeyModifiers::SHIFT
        && matches!(k.codes, crokey::OneToThree::One(KeyCode::Char(c)) if c.is_ascii_alphabetic())
}

/// Shift is part of a letter's identity (`G`), but terminals also report it
/// for symbols typed with shift (`?`), where it carries no meaning.
fn normalise(k: KeyCombination) -> KeyCombination {
    let mut k = k.normalized();
    if let crokey::OneToThree::One(KeyCode::Char(c)) = k.codes
        && !c.is_ascii_alphabetic()
    {
        k.modifiers.remove(KeyModifiers::SHIFT);
    }
    k
}

pub fn from_event(event: KeyEvent) -> KeyCombination {
    normalise(KeyCombination::from(event))
}

fn label(k: KeyCombination) -> String {
    let mut out = String::new();
    if k.modifiers.contains(KeyModifiers::CONTROL) {
        out.push_str("ctrl-");
    }
    if k.modifiers.contains(KeyModifiers::ALT) {
        out.push_str("alt-");
    }
    let code = match k.codes {
        crokey::OneToThree::One(code) => code,
        _ => return format!("{out}{k}"),
    };
    let name = match code {
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(c) if k.modifiers.contains(KeyModifiers::SHIFT) => {
            c.to_ascii_uppercase().to_string()
        }
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "enter".into(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::BackTab => "shift-tab".into(),
        KeyCode::Backspace => "backspace".into(),
        KeyCode::Left => "←".into(),
        KeyCode::Right => "→".into(),
        KeyCode::Up => "↑".into(),
        KeyCode::Down => "↓".into(),
        KeyCode::PageDown => "pgdn".into(),
        KeyCode::PageUp => "pgup".into(),
        KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(),
        KeyCode::F(n) => format!("f{n}"),
        other => format!("{other:?}").to_lowercase(),
    };
    if k.modifiers.contains(KeyModifiers::SHIFT)
        && !matches!(code, KeyCode::Char(_) | KeyCode::BackTab)
    {
        out.push_str("shift-");
    }
    out + &name
}

/// What a key did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feed {
    Action(Action),
    /// The keys so far start a longer binding; wait for the next one.
    Pending(Keys),
    /// The keys so far started a sequence that doesn't exist.
    Unbound(Keys),
    /// A single key with no binding.
    Ignored,
}

/// Bindings per context, in the order they were made: the first binding for
/// an action is the one key bars and help show.
#[derive(Debug, Clone, Default)]
pub struct Keymap {
    bindings: Vec<(Context, Keys, Action)>,
}

impl Keymap {
    /// abc's default bindings: Vim motions and a space leader.
    pub fn defaults() -> Self {
        use Action::*;
        use Context::*;
        let defaults: &[(Context, &str, Action)] = &[
            (Global, "ctrl-p", Switcher),
            (Global, "tab", LastWorktree),
            (Global, "h", FocusLeft),
            (Global, "left", FocusLeft),
            (Global, "l", FocusRight),
            (Global, "right", FocusRight),
            (Global, "?", Help),
            (Global, "q", Quit),
            (Global, "r", Retry),
            (Global, "space f", FindFile),
            (Global, "space w", FindWorktree),
            (Global, "space r a", AddRepo),
            (Global, "space r d", RemoveRepo),
            (Global, "space ?", Help),
            (Sidebar, "j", Down),
            (Sidebar, "down", Down),
            (Sidebar, "k", Up),
            (Sidebar, "up", Up),
            (Sidebar, "g g", First),
            (Sidebar, "shift-g", Last),
            (Sidebar, "enter", Open),
            (Sidebar, "o", Open),
            (Explorer, "j", Down),
            (Explorer, "down", Down),
            (Explorer, "k", Up),
            (Explorer, "up", Up),
            (Explorer, "g g", First),
            (Explorer, "shift-g", Last),
            (Explorer, "enter", Open),
            (Explorer, "o", Open),
            (Viewer, "h", Left),
            (Viewer, "left", Left),
            (Viewer, "j", Down),
            (Viewer, "down", Down),
            (Viewer, "k", Up),
            (Viewer, "up", Up),
            (Viewer, "l", Right),
            (Viewer, "right", Right),
            (Viewer, "w", WordForward),
            (Viewer, "b", WordBackward),
            (Viewer, "e", WordEnd),
            (Viewer, "shift-w", BigWordForward),
            (Viewer, "shift-b", BigWordBackward),
            (Viewer, "shift-e", BigWordEnd),
            (Viewer, "0", LineStart),
            (Viewer, "home", LineStart),
            (Viewer, "^", FirstNonBlank),
            (Viewer, "$", LineEnd),
            (Viewer, "end", LineEnd),
            (Viewer, "ctrl-d", HalfPageDown),
            (Viewer, "ctrl-u", HalfPageUp),
            (Viewer, "ctrl-f", PageDown),
            (Viewer, "pagedown", PageDown),
            (Viewer, "ctrl-b", PageUp),
            (Viewer, "pageup", PageUp),
            (Viewer, "ctrl-e", ScrollDown),
            (Viewer, "ctrl-y", ScrollUp),
            (Viewer, "g g", First),
            (Viewer, "shift-g", Last),
            (Viewer, "shift-h", ScreenTop),
            (Viewer, "shift-m", ScreenMiddle),
            (Viewer, "shift-l", ScreenBottom),
            (Viewer, "z t", CursorLineTop),
            (Viewer, "z z", CursorLineMiddle),
            (Viewer, "z b", CursorLineBottom),
            (Viewer, "}", ParagraphForward),
            (Viewer, "{", ParagraphBackward),
            (Viewer, "%", MatchingBracket),
            (Viewer, "f", FindForward),
            (Viewer, "shift-f", FindBackward),
            (Viewer, "t", TillForward),
            (Viewer, "shift-t", TillBackward),
            (Viewer, ";", RepeatFind),
            (Viewer, ",", RepeatFindBack),
            (Viewer, "ctrl-o", JumpBack),
            (Viewer, "esc", FocusLeft),
        ];
        let mut map = Self::default();
        for (context, keys, action) in defaults {
            map.bind(*context, keys, *action)
                .expect("default bindings are valid");
        }
        map
    }

    /// Binds `keys` in `context`. A sequence that is a prefix of another in
    /// the same context (or its global layer) is an error: it could never wait
    /// for the longer one.
    pub fn bind(&mut self, context: Context, keys: &str, action: Action) -> Result<(), String> {
        let keys: Keys = keys.parse()?;
        for (other, _) in self.visible(context) {
            if other != &keys && (other.0.starts_with(&keys.0) || keys.0.starts_with(&other.0)) {
                return Err(format!(
                    "{keys} and {other} overlap: one is the start of the other"
                ));
            }
        }
        match self
            .bindings
            .iter_mut()
            .find(|(c, k, _)| *c == context && *k == keys)
        {
            Some(binding) => binding.2 = action,
            None => self.bindings.push((context, keys, action)),
        }
        Ok(())
    }

    /// Bindings that apply in `context`: its own, then global ones it doesn't override.
    fn visible(&self, context: Context) -> impl Iterator<Item = (&Keys, &Action)> {
        let own = move |c: &Context| *c == context;
        let overridden = move |k: &Keys| self.bindings.iter().any(|(c, ok, _)| own(c) && ok == k);
        let pane = self.bindings.iter().filter(move |(c, _, _)| own(c));
        let global = self.bindings.iter().filter(move |(c, k, _)| {
            context != Context::Global && *c == Context::Global && !overridden(k)
        });
        pane.chain(global).map(|(_, k, a)| (k, a))
    }

    /// Feeds one key. `pending` holds the keys of an unfinished sequence and
    /// is updated in place.
    pub fn feed(
        &self,
        context: Context,
        pending: &mut Vec<KeyCombination>,
        key: KeyCombination,
    ) -> Feed {
        pending.push(key);
        let sequence = Keys(pending.clone());
        let mut longer = false;
        for (keys, action) in self.visible(context) {
            if keys == &sequence {
                pending.clear();
                return Feed::Action(*action);
            }
            longer |= keys.0.starts_with(&sequence.0);
        }
        if longer {
            return Feed::Pending(sequence);
        }
        pending.clear();
        if sequence.0.len() > 1 {
            Feed::Unbound(sequence)
        } else {
            Feed::Ignored
        }
    }

    /// The first binding for `action` in `context`, for key bars and help.
    pub fn keys_for(&self, context: Context, action: Action) -> Option<&Keys> {
        self.visible(context)
            .find(|(_, a)| **a == action)
            .map(|(k, _)| k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::KeyEventKind;

    fn press(code: KeyCode, modifiers: KeyModifiers) -> KeyCombination {
        from_event(KeyEvent::new_with_kind(
            code,
            modifiers,
            KeyEventKind::Press,
        ))
    }

    fn ch(c: char) -> KeyCombination {
        press(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn feed_all(map: &Keymap, context: Context, keys: &[KeyCombination]) -> Feed {
        let mut pending = Vec::new();
        let mut last = Feed::Ignored;
        for k in keys {
            last = map.feed(context, &mut pending, *k);
        }
        last
    }

    #[test]
    fn a_single_key_runs_its_action() {
        let map = Keymap::defaults();
        assert_eq!(
            feed_all(&map, Context::Sidebar, &[ch('j')]),
            Feed::Action(Action::Down)
        );
        assert_eq!(
            feed_all(&map, Context::Sidebar, &[ch('q')]),
            Feed::Action(Action::Quit)
        );
    }

    #[test]
    fn the_leader_waits_for_the_rest_of_the_sequence() {
        let map = Keymap::defaults();
        let mut pending = Vec::new();
        assert!(matches!(
            map.feed(Context::Explorer, &mut pending, ch(' ')),
            Feed::Pending(_)
        ));
        assert!(matches!(
            map.feed(Context::Explorer, &mut pending, ch('r')),
            Feed::Pending(_)
        ));
        assert_eq!(
            map.feed(Context::Explorer, &mut pending, ch('a')),
            Feed::Action(Action::AddRepo)
        );
        assert!(pending.is_empty());
    }

    #[test]
    fn a_wrong_key_after_a_prefix_is_unbound_and_resets() {
        let map = Keymap::defaults();
        let mut pending = Vec::new();
        map.feed(Context::Viewer, &mut pending, ch(' '));
        let fed = map.feed(Context::Viewer, &mut pending, ch('x'));
        assert_eq!(fed.clone(), Feed::Unbound("space x".parse().unwrap()));
        assert!(pending.is_empty());
        assert_eq!(
            map.feed(Context::Viewer, &mut pending, ch('j')),
            Feed::Action(Action::Down)
        );
    }

    #[test]
    fn an_unbound_single_key_is_ignored() {
        assert_eq!(
            feed_all(&Keymap::defaults(), Context::Sidebar, &[ch('z')]),
            Feed::Ignored
        );
    }

    #[test]
    fn gg_and_shifted_g_are_different_keys() {
        let map = Keymap::defaults();
        assert_eq!(
            feed_all(&map, Context::Viewer, &[ch('g'), ch('g')]),
            Feed::Action(Action::First)
        );
        // Terminals send `G` with or without the shift modifier.
        let upper = press(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(
            feed_all(&map, Context::Viewer, &[upper]),
            Feed::Action(Action::Last)
        );
        let upper = press(KeyCode::Char('G'), KeyModifiers::NONE);
        assert_eq!(
            feed_all(&map, Context::Viewer, &[upper]),
            Feed::Action(Action::Last)
        );
    }

    #[test]
    fn a_shifted_symbol_matches_its_plain_binding() {
        let question = press(KeyCode::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(
            feed_all(&Keymap::defaults(), Context::Sidebar, &[question]),
            Feed::Action(Action::Help)
        );
    }

    #[test]
    fn pane_bindings_override_global_ones() {
        let map = Keymap::defaults();
        // esc in the viewer goes back to the explorer; elsewhere it's unbound.
        let esc = press(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(
            feed_all(&map, Context::Viewer, &[esc]),
            Feed::Action(Action::FocusLeft)
        );
        assert_eq!(feed_all(&map, Context::Sidebar, &[esc]), Feed::Ignored);
    }

    #[test]
    fn the_viewer_moves_its_cursor_with_vim_keys() {
        let map = Keymap::defaults();
        let viewer = |keys: &[KeyCombination]| feed_all(&map, Context::Viewer, keys);
        assert_eq!(viewer(&[ch('h')]), Feed::Action(Action::Left));
        assert_eq!(viewer(&[ch('0')]), Feed::Action(Action::LineStart));
        assert_eq!(viewer(&[ch('$')]), Feed::Action(Action::LineEnd));
        assert_eq!(viewer(&[ch('{')]), Feed::Action(Action::ParagraphBackward));
        assert_eq!(
            viewer(&[ch('z'), ch('z')]),
            Feed::Action(Action::CursorLineMiddle)
        );
        let shifted = |c| press(KeyCode::Char(c), KeyModifiers::SHIFT);
        assert_eq!(
            viewer(&[shifted('W')]),
            Feed::Action(Action::BigWordForward)
        );
        assert_eq!(viewer(&[shifted('L')]), Feed::Action(Action::ScreenBottom));
        assert_eq!(
            viewer(&[shifted('%')]),
            Feed::Action(Action::MatchingBracket)
        );
        // Outside the viewer, h and l still move between panes.
        assert_eq!(
            feed_all(&map, Context::Explorer, &[ch('h')]),
            Feed::Action(Action::FocusLeft)
        );
    }

    #[test]
    fn a_binding_that_is_the_start_of_another_is_rejected() {
        let mut map = Keymap::defaults();
        assert!(map.bind(Context::Global, "space r", Action::Quit).is_err());
        assert!(map.bind(Context::Sidebar, "g", Action::First).is_err());
    }

    #[test]
    fn labels_read_like_the_key_bar() {
        let map = Keymap::defaults();
        let label = |c, a| short(map.keys_for(c, a).unwrap());
        assert_eq!(label(Context::Global, Action::Switcher), "^p");
        assert_eq!(label(Context::Sidebar, Action::First), "gg");
        assert_eq!(label(Context::Sidebar, Action::Last), "G");
        assert_eq!(label(Context::Global, Action::AddRepo), "space r a");
        assert_eq!(label(Context::Viewer, Action::HalfPageDown), "^d");
        assert_eq!(label(Context::Global, Action::Help), "?");
    }
}
