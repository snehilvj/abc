//! Syntax highlighting with tree-sitter, for Rust, TypeScript, TSX and Python.
//! A file is highlighted once, off the UI thread, into spans per line; the
//! viewer draws only the lines on screen from them.

use std::sync::OnceLock;

use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    Rust,
    TypeScript,
    Tsx,
    Python,
    /// Anything else: shown in the default colour.
    Plain,
}

impl Lang {
    /// The language of a file, by its name.
    pub fn of(name: &str) -> Self {
        match name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("rs") => Self::Rust,
            Some("ts" | "mts" | "cts") => Self::TypeScript,
            Some("tsx") => Self::Tsx,
            Some("py" | "pyi") => Self::Python,
            _ => Self::Plain,
        }
    }

    /// As the viewer's header shows it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "ts",
            Self::Tsx => "tsx",
            Self::Python => "python",
            Self::Plain => "plain",
        }
    }
}

/// What a span of code is, as the theme colours it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Keyword,
    String,
    Comment,
    Type,
    Function,
    Constant,
    Property,
    Punctuation,
}

/// tree-sitter highlight names abc colours, and their kinds. A capture such as
/// `function.method` takes the longest name here that it starts with.
const NAMES: &[(&str, Kind)] = &[
    ("keyword", Kind::Keyword),
    ("include", Kind::Keyword),
    ("conditional", Kind::Keyword),
    ("repeat", Kind::Keyword),
    ("exception", Kind::Keyword),
    ("storageclass", Kind::Keyword),
    ("string", Kind::String),
    ("escape", Kind::String),
    ("character", Kind::String),
    ("comment", Kind::Comment),
    ("type", Kind::Type),
    ("constructor", Kind::Type),
    ("attribute", Kind::Type),
    ("function", Kind::Function),
    ("method", Kind::Function),
    ("macro", Kind::Function),
    ("constant", Kind::Constant),
    ("number", Kind::Constant),
    ("boolean", Kind::Constant),
    ("property", Kind::Property),
    ("punctuation", Kind::Punctuation),
    ("operator", Kind::Punctuation),
];

/// A highlighted part of one line, by byte offsets within the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub kind: Kind,
}

fn config(lang: Lang) -> Option<&'static HighlightConfiguration> {
    fn build(
        language: tree_sitter::Language,
        name: &str,
        queries: &[&str],
    ) -> HighlightConfiguration {
        let highlights = queries.join("\n");
        let mut config = HighlightConfiguration::new(language, name, &highlights, "", "")
            .unwrap_or_else(|e| panic!("{name} highlight queries: {e}"));
        let names: Vec<&str> = NAMES.iter().map(|(n, _)| *n).collect();
        config.configure(&names);
        config
    }
    static RUST: OnceLock<HighlightConfiguration> = OnceLock::new();
    static TYPESCRIPT: OnceLock<HighlightConfiguration> = OnceLock::new();
    static TSX: OnceLock<HighlightConfiguration> = OnceLock::new();
    static PYTHON: OnceLock<HighlightConfiguration> = OnceLock::new();
    // TypeScript's own query covers only TypeScript syntax; upstream layers the
    // JavaScript queries under it (ADR 0003). Earlier patterns win.
    Some(match lang {
        Lang::Rust => RUST.get_or_init(|| {
            build(
                tree_sitter_rust::LANGUAGE.into(),
                "rust",
                &[tree_sitter_rust::HIGHLIGHTS_QUERY],
            )
        }),
        Lang::TypeScript => TYPESCRIPT.get_or_init(|| {
            build(
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
                "typescript",
                &[
                    tree_sitter_typescript::HIGHLIGHTS_QUERY,
                    tree_sitter_javascript::HIGHLIGHT_QUERY,
                ],
            )
        }),
        Lang::Tsx => TSX.get_or_init(|| {
            build(
                tree_sitter_typescript::LANGUAGE_TSX.into(),
                "tsx",
                &[
                    tree_sitter_typescript::HIGHLIGHTS_QUERY,
                    tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
                    tree_sitter_javascript::HIGHLIGHT_QUERY,
                ],
            )
        }),
        Lang::Python => PYTHON.get_or_init(|| {
            build(
                tree_sitter_python::LANGUAGE.into(),
                "python",
                &[tree_sitter_python::HIGHLIGHTS_QUERY],
            )
        }),
        Lang::Plain => return None,
    })
}

/// Spans for each line of `source`, or None for a language abc doesn't highlight.
pub fn highlight(source: &str, lang: Lang) -> Option<Vec<Vec<Span>>> {
    let config = config(lang)?;
    // Byte offset where each line starts, so a span can be split across lines.
    let starts: Vec<usize> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let mut lines: Vec<Vec<Span>> = vec![Vec::new(); starts.len()];
    let mut highlighter = Highlighter::new();
    let events = highlighter
        .highlight(config, source.as_bytes(), None, None, |_| None)
        .ok()?;
    let mut stack: Vec<Kind> = Vec::new();
    for event in events {
        match event.ok()? {
            HighlightEvent::HighlightStart(h) => stack.push(NAMES[h.0].1),
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let Some(&kind) = stack.last() else { continue };
                let mut line = starts.partition_point(|&s| s <= start) - 1;
                let mut from = start;
                while from < end && line < starts.len() {
                    let line_end = starts.get(line + 1).map_or(source.len(), |next| next - 1);
                    let to = end.min(line_end);
                    if to > from {
                        lines[line].push(Span {
                            start: from - starts[line],
                            end: to - starts[line],
                            kind,
                        });
                    }
                    line += 1;
                    from = starts.get(line).copied().unwrap_or(end);
                }
            }
        }
    }
    Some(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text of each span on `line`, with its kind.
    fn spans(source: &str, lang: Lang, line: usize) -> Vec<(String, Kind)> {
        let lines: Vec<&str> = source.split('\n').collect();
        highlight(source, lang).unwrap()[line]
            .iter()
            .map(|s| (lines[line][s.start..s.end].to_string(), s.kind))
            .collect()
    }

    fn has(spans: &[(String, Kind)], text: &str, kind: Kind) -> bool {
        spans.iter().any(|(t, k)| t == text && *k == kind)
    }

    #[test]
    fn languages_come_from_file_names() {
        assert_eq!(Lang::of("main.rs"), Lang::Rust);
        assert_eq!(Lang::of("verify.ts"), Lang::TypeScript);
        assert_eq!(Lang::of("App.TSX"), Lang::Tsx);
        assert_eq!(Lang::of("score.py"), Lang::Python);
        assert_eq!(Lang::of("README.md"), Lang::Plain);
        assert_eq!(Lang::of("Makefile"), Lang::Plain);
        assert!(highlight("anything", Lang::Plain).is_none());
    }

    #[test]
    fn rust_keywords_strings_comments_and_functions() {
        let src = "// a comment\nfn main() {\n    let s = \"hi\";\n    run(s, 42);\n}";
        assert!(has(
            &spans(src, Lang::Rust, 0),
            "// a comment",
            Kind::Comment
        ));
        let line1 = spans(src, Lang::Rust, 1);
        assert!(has(&line1, "fn", Kind::Keyword));
        assert!(has(&line1, "main", Kind::Function));
        assert!(has(&spans(src, Lang::Rust, 2), "\"hi\"", Kind::String));
        let line3 = spans(src, Lang::Rust, 3);
        assert!(has(&line3, "run", Kind::Function));
        assert!(has(&line3, "42", Kind::Constant));
    }

    #[test]
    fn typescript_gets_javascript_keywords_and_its_own_types() {
        let src =
            "export function verify(input: GstInput): string {\n  const a = `x`\n  return a\n}";
        let line0 = spans(src, Lang::TypeScript, 0);
        assert!(has(&line0, "export", Kind::Keyword));
        assert!(has(&line0, "function", Kind::Keyword));
        assert!(has(&line0, "verify", Kind::Function));
        assert!(has(&line0, "string", Kind::Type));
        let line1 = spans(src, Lang::TypeScript, 1);
        assert!(has(&line1, "const", Kind::Keyword));
        assert!(has(&line1, "`x`", Kind::String));
        assert!(has(
            &spans(src, Lang::TypeScript, 2),
            "return",
            Kind::Keyword
        ));
    }

    #[test]
    fn tsx_highlights_jsx() {
        let src = "export const App = () => <Button label=\"go\" />";
        let line = spans(src, Lang::Tsx, 0);
        assert!(has(&line, "export", Kind::Keyword));
        assert!(has(&line, "\"go\"", Kind::String));
        assert!(line.iter().any(|(t, _)| t == "Button"));
    }

    #[test]
    fn python_keywords_strings_and_functions() {
        let src = "def score(x):\n    return len(\"ab\") + 1  # note";
        let line0 = spans(src, Lang::Python, 0);
        assert!(has(&line0, "def", Kind::Keyword));
        assert!(has(&line0, "score", Kind::Function));
        let line1 = spans(src, Lang::Python, 1);
        assert!(has(&line1, "return", Kind::Keyword));
        assert!(has(&line1, "\"ab\"", Kind::String));
        assert!(has(&line1, "# note", Kind::Comment));
    }

    #[test]
    fn a_comment_across_lines_is_split_per_line() {
        let src = "/* one\ntwo */ fn f() {}";
        assert_eq!(
            spans(src, Lang::Rust, 0),
            [("/* one".to_string(), Kind::Comment)]
        );
        assert!(has(&spans(src, Lang::Rust, 1), "two */", Kind::Comment));
    }

    #[test]
    fn spans_stay_within_their_lines() {
        let src = "fn a() {}\n\nlet s = \"multi\nline\";\n";
        let lines: Vec<&str> = src.split('\n').collect();
        for (i, spans) in highlight(src, Lang::Rust).unwrap().iter().enumerate() {
            for s in spans {
                assert!(
                    s.start < s.end && s.end <= lines[i].len(),
                    "line {i}: {s:?}"
                );
            }
        }
    }
}
