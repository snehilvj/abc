//! Finding text in the viewer: `/`, `n`, `N`, `*` and `#`. Matching itself
//! is `abc_core::text`'s, shared with find in files: plain text, smart-case.

use crate::motion::Pos;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Search {
    pub pattern: String,
    /// The way `n` goes; `N` goes the other.
    pub forward: bool,
    /// Only whole words match, as with `*` and `#`. Case always counts then.
    pub whole_word: bool,
}

/// Where a search landed, and whether it went round an end of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub at: Pos,
    pub wrapped: bool,
}

impl Search {
    pub fn new(pattern: impl Into<String>, forward: bool) -> Self {
        Self {
            pattern: pattern.into(),
            forward,
            whole_word: false,
        }
    }

    /// `*` and `#`: `word`, whole.
    pub fn word(word: impl Into<String>, forward: bool) -> Self {
        Self {
            whole_word: true,
            ..Self::new(word, forward)
        }
    }

    /// The matches on `line`, as columns: each one's first, and one past its last.
    pub fn matches(&self, line: &str) -> Vec<(usize, usize)> {
        abc_core::text::matches(&self.pattern, self.whole_word, line)
    }

    /// The next match from `p`, `forward` or back, going round the file's ends.
    pub fn next(&self, lines: &[String], p: Pos, forward: bool) -> Option<Found> {
        let n = lines.len();
        if n == 0 {
            return None;
        }
        // The cursor's line comes up twice: first the part past the cursor,
        // then, after going round, the part before it.
        for step in 0..=n {
            let line = if forward {
                (p.line + step) % n
            } else {
                (p.line + n - step % n) % n
            };
            let matches = self.matches(&lines[line]);
            let found = match (step, forward) {
                (0, true) => matches.iter().find(|m| m.0 > p.col),
                (0, false) => matches.iter().rev().find(|m| m.0 < p.col),
                (s, true) if s == n => matches.iter().find(|m| m.0 <= p.col),
                (s, false) if s == n => matches.iter().rev().find(|m| m.0 >= p.col),
                (_, true) => matches.first(),
                (_, false) => matches.last(),
            };
            if let Some(m) = found {
                let wrapped = if forward {
                    p.line + step >= n
                } else {
                    step > p.line
                };
                return Some(Found {
                    at: Pos::new(line, m.0),
                    wrapped,
                });
            }
        }
        None
    }

    /// Which match `p` is, counting from the top, and how many there are.
    pub fn count(&self, lines: &[String], p: Pos) -> (usize, usize) {
        let (mut upto, mut total) = (0, 0);
        for (i, line) in lines.iter().enumerate() {
            for (start, _) in self.matches(line) {
                total += 1;
                if Pos::new(i, start) <= p {
                    upto += 1;
                }
            }
        }
        (upto, total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(s: &str) -> Vec<String> {
        s.split('\n').map(String::from).collect()
    }

    #[test]
    fn lower_case_matches_either_case_and_a_capital_makes_case_count() {
        let line = "Gstin gstin GSTIN";
        assert_eq!(
            Search::new("gstin", true).matches(line),
            [(0, 5), (6, 11), (12, 17)]
        );
        assert_eq!(Search::new("Gstin", true).matches(line), [(0, 5)]);
    }

    #[test]
    fn a_word_search_skips_the_word_inside_others() {
        let line = "id ids _id id";
        assert_eq!(Search::word("id", true).matches(line), [(0, 2), (11, 13)]);
    }

    #[test]
    fn n_goes_round_the_ends_of_the_file() {
        let t = text("a gstin\nnothing\ngstin b");
        let s = Search::new("gstin", true);
        let at = |line, col| Pos::new(line, col);
        let found = |p, forward| s.next(&t, p, forward).map(|f| (f.at, f.wrapped));
        assert_eq!(found(at(0, 0), true), Some((at(0, 2), false)));
        assert_eq!(found(at(0, 2), true), Some((at(2, 0), false)));
        assert_eq!(found(at(2, 0), true), Some((at(0, 2), true)));
        assert_eq!(found(at(2, 0), false), Some((at(0, 2), false)));
        assert_eq!(found(at(0, 2), false), Some((at(2, 0), true)));
    }

    #[test]
    fn a_lone_match_is_found_again_after_going_round() {
        let t = text("one gstin");
        let found = Search::new("gstin", true).next(&t, Pos::new(0, 4), true);
        assert_eq!(
            found,
            Some(Found {
                at: Pos::new(0, 4),
                wrapped: true
            })
        );
        assert_eq!(
            Search::new("none", true).next(&t, Pos::new(0, 0), true),
            None
        );
    }

    #[test]
    fn matches_are_counted_from_the_top() {
        let t = text("gstin\nx gstin gstin");
        assert_eq!(Search::new("gstin", true).count(&t, Pos::new(1, 8)), (3, 3));
        assert_eq!(Search::new("gstin", true).count(&t, Pos::new(1, 2)), (2, 3));
    }
}
