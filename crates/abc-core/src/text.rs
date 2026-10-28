//! Finding text in a line, as the viewer's `/` and find in files both do:
//! plain text rather than regular expressions, and smart-case, as with Vim's
//! `smartcase`: all lower case matches either case; a capital makes case count.

/// The matches of `pattern` on `line`, as character columns: each one's
/// first, and one past its last. With `whole_word`, only matches with no
/// letter, digit or `_` either side count, and case always counts.
pub fn matches(pattern: &str, whole_word: bool, line: &str) -> Vec<(usize, usize)> {
    let pattern: Vec<char> = pattern.chars().collect();
    if pattern.is_empty() {
        return Vec::new();
    }
    let chars: Vec<char> = line.chars().collect();
    let case = whole_word || pattern.iter().any(|c| c.is_uppercase());
    let same = |a: char, b: char| a == b || (!case && a.to_lowercase().eq(b.to_lowercase()));
    let word = |i: usize| {
        chars
            .get(i)
            .is_some_and(|c| c.is_alphanumeric() || *c == '_')
    };
    (0..(chars.len() + 1).saturating_sub(pattern.len()))
        .filter(|&i| {
            pattern
                .iter()
                .enumerate()
                .all(|(j, p)| same(chars[i + j], *p))
        })
        .filter(|&i| {
            let end = i + pattern.len();
            !whole_word || !((i > 0 && word(i - 1)) || word(end))
        })
        .map(|i| (i, i + pattern.len()))
        .collect()
}

/// Whether `pattern` matches anywhere on `line`.
pub fn contains(pattern: &str, line: &str) -> bool {
    !matches(pattern, false, line).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_case_matches_either_case_and_a_capital_makes_case_count() {
        assert_eq!(matches("gst", false, "GstClient gst"), [(0, 3), (10, 13)]);
        assert_eq!(matches("Gst", false, "GstClient gst"), [(0, 3)]);
        assert_eq!(matches("gst", true, "gst gstin _gst"), [(0, 3)]);
        assert!(!contains("", "anything"));
    }
}
