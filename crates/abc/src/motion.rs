//! Vim motions over read-only text: pure functions from the lines and a
//! position to a new position. Columns count characters; tabs are already
//! spaces by the time text gets here.

/// A place in the text: a line, and a character within it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

impl Pos {
    pub const fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

fn len(lines: &[String], line: usize) -> usize {
    lines.get(line).map_or(0, |l| l.chars().count())
}

fn char_at(lines: &[String], p: Pos) -> Option<char> {
    lines.get(p.line)?.chars().nth(p.col)
}

/// The last column the cursor can sit on: the last character, or 0 on an empty line.
pub fn last_col(lines: &[String], line: usize) -> usize {
    len(lines, line).saturating_sub(1)
}

/// `p` moved onto real text.
pub fn clamp(lines: &[String], p: Pos) -> Pos {
    let line = p.line.min(lines.len().saturating_sub(1));
    Pos::new(line, p.col.min(last_col(lines, line)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A line with nothing on it: Vim stops on these as if they were words.
    Empty,
    Blank,
    Punctuation,
    Word,
}

fn class(lines: &[String], p: Pos, big: bool) -> Class {
    match char_at(lines, p) {
        None => Class::Empty,
        Some(c) if c.is_whitespace() => Class::Blank,
        Some(_) if big => Class::Word,
        Some(c) if c.is_alphanumeric() || c == '_' => Class::Word,
        Some(_) => Class::Punctuation,
    }
}

fn next(lines: &[String], p: Pos) -> Option<Pos> {
    if p.col + 1 < len(lines, p.line) {
        Some(Pos::new(p.line, p.col + 1))
    } else if p.line + 1 < lines.len() {
        Some(Pos::new(p.line + 1, 0))
    } else {
        None
    }
}

fn prev(lines: &[String], p: Pos) -> Option<Pos> {
    if p.col > 0 {
        Some(Pos::new(p.line, p.col - 1))
    } else if p.line > 0 {
        Some(Pos::new(p.line - 1, last_col(lines, p.line - 1)))
    } else {
        None
    }
}

/// `w` (or `W` with `big`): the start of the next word, or an empty line.
pub fn word_forward(lines: &[String], p: Pos, big: bool) -> Pos {
    let start = class(lines, p, big);
    let mut q = p;
    if matches!(start, Class::Word | Class::Punctuation) {
        loop {
            match next(lines, q) {
                None => return Pos::new(q.line, last_col(lines, q.line)),
                Some(n) if n.line != q.line || class(lines, n, big) != start => {
                    q = n;
                    break;
                }
                Some(n) => q = n,
            }
        }
    } else {
        match next(lines, q) {
            Some(n) => q = n,
            None => return q,
        }
    }
    while class(lines, q, big) == Class::Blank {
        match next(lines, q) {
            Some(n) => q = n,
            None => return q,
        }
    }
    q
}

/// `e` (or `E`): the end of this word, or of the next one if already at an end.
pub fn word_end(lines: &[String], p: Pos, big: bool) -> Pos {
    let Some(mut q) = next(lines, p) else {
        return p;
    };
    while matches!(class(lines, q, big), Class::Blank | Class::Empty) {
        match next(lines, q) {
            Some(n) => q = n,
            None => return q,
        }
    }
    let c = class(lines, q, big);
    while let Some(n) = next(lines, q) {
        if n.line != q.line || class(lines, n, big) != c {
            break;
        }
        q = n;
    }
    q
}

/// `b` (or `B`): the start of this word, or of the previous one if already at a start.
pub fn word_backward(lines: &[String], p: Pos, big: bool) -> Pos {
    let Some(mut q) = prev(lines, p) else {
        return p;
    };
    while class(lines, q, big) == Class::Blank {
        match prev(lines, q) {
            Some(n) => q = n,
            None => return q,
        }
    }
    let c = class(lines, q, big);
    if c == Class::Empty {
        return q;
    }
    while let Some(n) = prev(lines, q) {
        if n.line != q.line || class(lines, n, big) != c {
            break;
        }
        q = n;
    }
    q
}

/// `^`: the first character that isn't blank.
pub fn first_non_blank(lines: &[String], line: usize) -> Pos {
    let col = lines
        .get(line)
        .and_then(|l| l.chars().position(|c| !c.is_whitespace()))
        .unwrap_or(0);
    Pos::new(line, col)
}

/// `f`/`F`/`t`/`T`: `target` on this line, forward or back, landing on it or just before it.
pub fn find_char(lines: &[String], p: Pos, target: char, forward: bool, till: bool) -> Option<Pos> {
    let chars: Vec<char> = lines.get(p.line)?.chars().collect();
    let col = if forward {
        let found = (p.col + 1..chars.len()).find(|&i| chars[i] == target)?;
        if till { found - 1 } else { found }
    } else {
        let found = (0..p.col).rev().find(|&i| chars[i] == target)?;
        if till { found + 1 } else { found }
    };
    Some(Pos::new(p.line, col))
}

/// An `f`, `F`, `t` or `T` with its character, kept for `;` and `,`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Find {
    pub target: char,
    pub forward: bool,
    pub till: bool,
}

impl Find {
    /// The same find the other way, for `,`.
    pub fn reversed(self) -> Self {
        Self {
            forward: !self.forward,
            ..self
        }
    }

    /// Where the find lands from `p`, `count` times over, or None if the line
    /// runs out first. `again` is for `;` and `,`: a till that stopped next to
    /// its character looks past it rather than stopping there again.
    pub fn apply(self, lines: &[String], p: Pos, count: usize, again: bool) -> Option<Pos> {
        let mut q = p;
        for i in 0..count.max(1) {
            let from = match (self.till && (again || i > 0), self.forward) {
                (false, _) => q,
                (true, true) => Pos::new(q.line, q.col + 1),
                (true, false) => Pos::new(q.line, q.col.checked_sub(1)?),
            };
            q = find_char(lines, from, self.target, self.forward, self.till)?;
        }
        Some(q)
    }
}

/// `%`: the bracket matching the one under the cursor, or the first one after it on the line.
pub fn matching_bracket(lines: &[String], p: Pos) -> Option<Pos> {
    const PAIRS: [(char, char); 3] = [('(', ')'), ('[', ']'), ('{', '}')];
    let chars: Vec<char> = lines.get(p.line)?.chars().collect();
    let col = (p.col..chars.len())
        .find(|&i| PAIRS.iter().any(|(o, c)| chars[i] == *o || chars[i] == *c))?;
    let at = chars[col];
    let (open, close, forward) = PAIRS.iter().find_map(|&(o, c)| {
        if at == o {
            Some((o, c, true))
        } else if at == c {
            Some((o, c, false))
        } else {
            None
        }
    })?;
    let mut depth = 0usize;
    let mut q = Pos::new(p.line, col);
    loop {
        match char_at(lines, q) {
            Some(c) if c == open => {
                depth = if forward {
                    depth + 1
                } else {
                    depth.checked_sub(1)?
                }
            }
            Some(c) if c == close => {
                depth = if forward {
                    depth.checked_sub(1)?
                } else {
                    depth + 1
                }
            }
            _ => {}
        }
        if depth == 0 {
            return Some(q);
        }
        q = if forward {
            next(lines, q)?
        } else {
            prev(lines, q)?
        };
    }
}

/// `}`: the next empty line, or the end of the text.
pub fn paragraph_forward(lines: &[String], p: Pos) -> Pos {
    let mut line = p.line + 1;
    // Leave the run of empty lines the cursor is in, then find the next one.
    while line < lines.len() && lines[line].trim().is_empty() {
        line += 1;
    }
    match (line..lines.len()).find(|&i| lines[i].trim().is_empty()) {
        Some(i) => Pos::new(i, 0),
        None => Pos::new(
            lines.len().saturating_sub(1),
            last_col(lines, lines.len().saturating_sub(1)),
        ),
    }
}

/// `{`: the previous empty line, or the start of the text.
pub fn paragraph_backward(lines: &[String], p: Pos) -> Pos {
    let mut line = p.line;
    while line > 0 && lines[line - 1].trim().is_empty() {
        line -= 1;
    }
    match (0..line.saturating_sub(1))
        .rev()
        .find(|&i| lines[i].trim().is_empty())
    {
        Some(i) => Pos::new(i, 0),
        None => Pos::new(0, 0),
    }
}

/// The word under the cursor, for `*` and `#`.
pub fn word_at(lines: &[String], p: Pos) -> Option<String> {
    let (start, end) = word_bounds(lines, p)?;
    Some(
        lines[p.line]
            .chars()
            .skip(start)
            .take(end - start)
            .collect(),
    )
}

/// The columns of the word under the cursor, or the next one on the line:
/// its first, and one past its last.
pub fn word_bounds(lines: &[String], p: Pos) -> Option<(usize, usize)> {
    let chars: Vec<char> = lines.get(p.line)?.chars().collect();
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let col = (p.col..chars.len()).find(|&i| is_word(chars[i]))?;
    let start = (0..=col).rev().take_while(|&i| is_word(chars[i])).last()?;
    let end = (col..chars.len())
        .take_while(|&i| is_word(chars[i]))
        .last()?
        + 1;
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Vec<String> {
        s.split('\n').map(String::from).collect()
    }

    fn at(line: usize, col: usize) -> Pos {
        Pos::new(line, col)
    }

    #[test]
    fn w_moves_by_words_and_punctuation_and_across_lines() {
        let t = text("let a = foo(bar);\n\n  next");
        assert_eq!(word_forward(&t, at(0, 0), false), at(0, 4));
        assert_eq!(word_forward(&t, at(0, 8), false), at(0, 11)); // foo -> (
        assert_eq!(word_forward(&t, at(0, 11), false), at(0, 12)); // ( -> bar
        assert_eq!(word_forward(&t, at(0, 15), false), at(1, 0)); // ); -> the empty line
        assert_eq!(word_forward(&t, at(1, 0), false), at(2, 2));
    }

    #[test]
    fn big_w_skips_punctuation_inside_words() {
        let t = text("a.b(c) next");
        assert_eq!(word_forward(&t, at(0, 0), true), at(0, 7));
        assert_eq!(word_forward(&t, at(0, 0), false), at(0, 1));
    }

    #[test]
    fn e_and_b_find_word_ends_and_starts() {
        let t = text("let alpha = beta;");
        assert_eq!(word_end(&t, at(0, 0), false), at(0, 2));
        assert_eq!(word_end(&t, at(0, 2), false), at(0, 8));
        assert_eq!(word_backward(&t, at(0, 14), false), at(0, 12));
        assert_eq!(word_backward(&t, at(0, 12), false), at(0, 10));
        assert_eq!(word_backward(&t, at(0, 0), false), at(0, 0));
    }

    #[test]
    fn b_crosses_lines_and_stops_at_empty_lines() {
        let t = text("one\n\ntwo");
        assert_eq!(word_backward(&t, at(2, 0), false), at(1, 0));
        assert_eq!(word_backward(&t, at(1, 0), false), at(0, 0));
    }

    #[test]
    fn f_and_t_find_characters_on_the_line() {
        let t = text("fn main(a, b) {}");
        assert_eq!(find_char(&t, at(0, 0), '(', true, false), Some(at(0, 7)));
        assert_eq!(find_char(&t, at(0, 0), '(', true, true), Some(at(0, 6)));
        assert_eq!(find_char(&t, at(0, 12), 'a', false, false), Some(at(0, 8)));
        assert_eq!(find_char(&t, at(0, 12), 'a', false, true), Some(at(0, 9)));
        assert_eq!(find_char(&t, at(0, 0), 'z', true, false), None);
    }

    #[test]
    fn t_next_to_its_character_stays_put_until_repeated() {
        let t = text("a(b(c");
        assert_eq!(find_char(&t, at(0, 0), '(', true, true), Some(at(0, 0)));
        assert_eq!(find_char(&t, at(0, 4), '(', false, true), Some(at(0, 4)));
        let find = Find {
            target: '(',
            forward: true,
            till: true,
        };
        assert_eq!(
            find.apply(&t, at(0, 0), 1, true),
            Some(at(0, 2)),
            "`;` looks past"
        );
        assert_eq!(
            find.apply(&t, at(0, 0), 2, false),
            Some(at(0, 2)),
            "a count finds the nth"
        );
        assert_eq!(
            find.reversed().apply(&t, at(0, 4), 1, false),
            Some(at(0, 4))
        );
        assert_eq!(find.apply(&t, at(0, 2), 1, true), None);
    }

    #[test]
    fn percent_matches_brackets_across_lines() {
        let t = text("fn f() {\n    g(a[1]);\n}");
        assert_eq!(matching_bracket(&t, at(0, 7)), Some(at(2, 0)));
        assert_eq!(matching_bracket(&t, at(2, 0)), Some(at(0, 7)));
        assert_eq!(
            matching_bracket(&t, at(1, 4)),
            Some(at(1, 10)),
            "from before a bracket, the first one on the line"
        );
        assert_eq!(matching_bracket(&t, at(1, 7)), Some(at(1, 9)));
    }

    #[test]
    fn braces_move_between_paragraphs() {
        let t = text("a\nb\n\nc\nd\n\n\ne");
        assert_eq!(paragraph_forward(&t, at(0, 0)), at(2, 0));
        assert_eq!(paragraph_forward(&t, at(2, 0)), at(5, 0));
        assert_eq!(paragraph_forward(&t, at(7, 0)), at(7, 0));
        assert_eq!(paragraph_backward(&t, at(4, 0)), at(2, 0));
        assert_eq!(paragraph_backward(&t, at(1, 0)), at(0, 0));
    }

    #[test]
    fn caret_skips_indentation_and_clamp_keeps_the_cursor_on_text() {
        let t = text("    indented\n\nx");
        assert_eq!(first_non_blank(&t, 0), at(0, 4));
        assert_eq!(clamp(&t, at(0, 99)), at(0, 11));
        assert_eq!(clamp(&t, at(1, 5)), at(1, 0));
        assert_eq!(clamp(&t, at(9, 0)), at(2, 0));
    }

    #[test]
    fn a_words_bounds_reach_back_from_the_cursor() {
        let t = text("let record = 1;");
        assert_eq!(word_bounds(&t, at(0, 6)), Some((4, 10)));
        assert_eq!(word_bounds(&t, at(0, 3)), Some((4, 10)));
    }

    #[test]
    fn the_word_under_the_cursor() {
        let t = text("let record = lookup(a);");
        assert_eq!(word_at(&t, at(0, 6)).as_deref(), Some("record"));
        assert_eq!(
            word_at(&t, at(0, 3)).as_deref(),
            Some("record"),
            "from a blank, the next word"
        );
        assert_eq!(word_at(&t, at(0, 22)), None);
    }
}
