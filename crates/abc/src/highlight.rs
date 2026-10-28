//! Syntax highlighting: tree-sitter for the languages abc has grammars for
//! (Rust, TypeScript, TSX, JavaScript, Python), and bat's syntax definitions,
//! through syntect, for every other kind of file they know (ADR 0006). A file
//! is highlighted once, off the UI thread, into spans per line; the viewer
//! draws only the lines on screen from them.

use std::collections::HashMap;
use std::sync::OnceLock;

use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxSet};
use syntect::util::LinesWithEndings;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    Rust,
    TypeScript,
    Tsx,
    /// JSX included.
    JavaScript,
    Python,
    /// One of bat's syntaxes, by name: "Markdown", "TOML", "HTML (Jinja2)", ...
    Syntax(&'static str),
    /// Anything else: shown in the default colour.
    Plain,
}

/// Files named for their language rather than by an extension bat knows.
const NAMED: &[(&str, &str)] = &[
    ("Cargo.lock", "TOML"),
    ("poetry.lock", "TOML"),
    ("uv.lock", "TOML"),
];

impl Lang {
    /// The language of a file, by its name.
    pub fn of(name: &str) -> Self {
        let file = name.rsplit('/').next().unwrap_or(name);
        match file
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("rs") => Self::Rust,
            Some("ts" | "mts" | "cts") => Self::TypeScript,
            Some("tsx") => Self::Tsx,
            Some("js" | "mjs" | "cjs" | "jsx") => Self::JavaScript,
            Some("py" | "pyi") => Self::Python,
            _ => syntax_for(file).map_or(Self::Plain, Self::Syntax),
        }
    }

    /// The language of a file by its name and text: a shebang names a
    /// script's, and an HTML file with Jinja's tags is Jinja.
    pub fn detect(name: &str, text: &str) -> Self {
        match Self::of(name) {
            Self::Plain => {
                let first = text.lines().next().unwrap_or("");
                syntaxes()
                    .find_syntax_by_first_line(first)
                    .filter(|s| s.name != "Plain Text")
                    .map_or(Self::Plain, |s| Self::Syntax(intern(&s.name)))
            }
            Self::Syntax("HTML") if jinja(text) => Self::Syntax("HTML (Jinja2)"),
            lang => lang,
        }
    }

    /// As the viewer's header shows it: short, such as "bash" for "Bourne
    /// Again Shell (bash)".
    pub fn label(self) -> String {
        match self {
            Self::Rust => "rust".into(),
            Self::TypeScript => "ts".into(),
            Self::Tsx => "tsx".into(),
            Self::JavaScript => "js".into(),
            Self::Python => "python".into(),
            Self::Syntax(name) => {
                let short = name
                    .split_once(" (")
                    .and_then(|(_, inner)| inner.strip_suffix(')'))
                    .unwrap_or(name);
                short.to_lowercase()
            }
            Self::Plain => "plain".into(),
        }
    }
}

/// bat's syntaxes, loaded once.
fn syntaxes() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(two_face::syntax::extra_newlines)
}

/// A syntax's name as it lives in the set, for the life of abc.
fn intern(name: &str) -> &'static str {
    syntaxes()
        .find_syntax_by_name(name)
        .map_or("Plain Text", |s| s.name.as_str())
}

/// bat's syntax for a file name: a compound extension (`html.j2`) first,
/// then the extension, then the whole name (`Makefile`).
fn syntax_for(file: &str) -> Option<&'static str> {
    if let Some((_, syntax)) = NAMED.iter().find(|(named, _)| *named == file) {
        return Some(syntax);
    }
    let set = syntaxes();
    let parts: Vec<&str> = file.split('.').collect();
    let compound = (parts.len() > 2).then(|| parts[parts.len() - 2..].join("."));
    let extension = (parts.len() > 1).then(|| parts[parts.len() - 1].to_string());
    [compound, extension, Some(file.to_string())]
        .into_iter()
        .flatten()
        .find_map(|candidate| set.find_syntax_by_extension(&candidate))
        .filter(|s| s.name != "Plain Text")
        .map(|s| s.name.as_str())
}

/// Whether HTML holds Jinja's statements or comments. `{{ }}` alone could be
/// any templating.
fn jinja(text: &str) -> bool {
    text.lines()
        .take(200)
        .any(|line| line.contains("{%") || line.contains("{#"))
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
    static JAVASCRIPT: OnceLock<HighlightConfiguration> = OnceLock::new();
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
        Lang::JavaScript => JAVASCRIPT.get_or_init(|| {
            build(
                tree_sitter_javascript::LANGUAGE.into(),
                "javascript",
                &[
                    tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
                    tree_sitter_javascript::HIGHLIGHT_QUERY,
                ],
            )
        }),
        Lang::Syntax(_) | Lang::Plain => return None,
    })
}

/// Spans for each line of `source`, or None for a language abc doesn't highlight.
pub fn highlight(source: &str, lang: Lang) -> Option<Vec<Vec<Span>>> {
    match lang {
        Lang::Syntax(name) => highlight_syntax(source, name),
        Lang::Plain => None,
        _ => highlight_tree(source, lang),
    }
}

/// bat's scopes abc colours, and their kinds. A scope such as
/// `keyword.control.flow.python` takes the longest prefix here, by whole
/// parts, and the innermost scope with one decides. Quotes and comment
/// markers go with what they mark.
const SCOPES: &[(&str, Kind)] = &[
    ("comment", Kind::Comment),
    ("punctuation.definition.comment", Kind::Comment),
    ("markup.quote", Kind::Comment),
    ("string", Kind::String),
    ("punctuation.definition.string", Kind::String),
    ("constant.character.escape", Kind::String),
    ("markup.raw", Kind::String),
    ("punctuation.definition.raw", Kind::String),
    ("constant", Kind::Constant),
    ("keyword", Kind::Keyword),
    ("keyword.operator", Kind::Punctuation),
    ("storage", Kind::Keyword),
    ("entity.name.tag", Kind::Keyword),
    ("markup.heading", Kind::Keyword),
    ("punctuation.definition.heading", Kind::Keyword),
    ("entity.name.function", Kind::Function),
    ("support.function", Kind::Function),
    ("variable.function", Kind::Function),
    ("markup.underline.link", Kind::Function),
    ("entity.name.type", Kind::Type),
    ("entity.name.class", Kind::Type),
    ("entity.name.section", Kind::Type),
    ("entity.other.inherited-class", Kind::Type),
    ("support.type", Kind::Type),
    ("support.class", Kind::Type),
    ("entity.other.attribute-name", Kind::Property),
    ("variable.other.member", Kind::Property),
    ("variable.other.property", Kind::Property),
    ("support.variable.property", Kind::Property),
    ("meta.mapping.key", Kind::Property),
    ("punctuation", Kind::Punctuation),
];

/// Scopes that colour everything inside them alike, whatever else is nested:
/// a heading's text as its `#`, a comment's links and keywords as the comment.
const WHOLE: &[&str] = &["comment", "markup.heading"];

/// Files with a line longer than this show plain: syntect's regexes are slow
/// on minified and generated lines.
const LONGEST_LINE: usize = 5_000;

/// The kind of a scope, by the longest prefix of whole parts in [`SCOPES`].
fn scope_kind(scope: Scope) -> Option<Kind> {
    let name = scope.build_string();
    SCOPES
        .iter()
        .filter(|(prefix, _)| {
            name == *prefix
                || name
                    .strip_prefix(prefix)
                    .is_some_and(|rest| rest.starts_with('.'))
        })
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, kind)| *kind)
}

/// Highlights with bat's syntax `name`, a line at a time.
fn highlight_syntax(source: &str, name: &str) -> Option<Vec<Vec<Span>>> {
    let set = syntaxes();
    let syntax = set.find_syntax_by_name(name)?;
    if source.split('\n').any(|line| line.len() > LONGEST_LINE) {
        return None;
    }
    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut kinds: HashMap<Scope, Option<Kind>> = HashMap::new();
    let mut kind_of = |scope: Scope| *kinds.entry(scope).or_insert_with(|| scope_kind(scope));
    let mut kind_now = |stack: &ScopeStack| {
        let scopes = stack.as_slice();
        let whole = scopes.iter().find(|scope| {
            let name = scope.build_string();
            WHOLE
                .iter()
                .any(|w| name == *w || name.strip_prefix(w).is_some_and(|r| r.starts_with('.')))
        });
        match whole {
            Some(scope) => kind_of(*scope),
            None => scopes.iter().rev().find_map(|scope| kind_of(*scope)),
        }
    };
    let mut lines: Vec<Vec<Span>> = Vec::new();
    for line in LinesWithEndings::from(source) {
        let text_end = line.strip_suffix('\n').map_or(line.len(), str::len);
        let mut spans: Vec<Span> = Vec::new();
        let mut push = |from: usize, to: usize, kind: Option<Kind>| {
            let to = to.min(text_end);
            let Some(kind) = kind.filter(|_| to > from) else {
                return;
            };
            match spans.last_mut() {
                Some(last) if last.kind == kind && last.end == from => last.end = to,
                _ => spans.push(Span {
                    start: from,
                    end: to,
                    kind,
                }),
            }
        };
        let mut from = 0;
        for (at, op) in state.parse_line(line, set).ok()? {
            push(from, at, kind_now(&stack));
            stack.apply(&op).ok()?;
            from = at;
        }
        push(from, text_end, kind_now(&stack));
        lines.push(spans);
    }
    // A final newline ends a last, empty line.
    lines.resize(source.split('\n').count(), Vec::new());
    Some(lines)
}

/// Highlights with a tree-sitter grammar.
fn highlight_tree(source: &str, lang: Lang) -> Option<Vec<Vec<Span>>> {
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
        assert_eq!(Lang::of("README.md"), Lang::Syntax("Markdown"));
        assert_eq!(Lang::of("Makefile"), Lang::Syntax("Makefile"));
        assert!(highlight("anything", Lang::Plain).is_none());
    }

    #[test]
    fn every_other_file_type_gets_one_of_bats_syntaxes() {
        for (name, syntax) in [
            ("Cargo.toml", "TOML"),
            ("Cargo.lock", "TOML"),
            (".github/check.yml", "YAML"),
            ("package.json", "JSON"),
            ("index.html", "HTML"),
            ("base.html.j2", "HTML (Jinja2)"),
            ("macros.jinja", "Jinja2"),
            (".gitignore", "Git Ignore"),
            ("Dockerfile", "Dockerfile"),
            ("deploy.sh", "Bourne Again Shell (bash)"),
            ("schema.SQL", "SQL"),
        ] {
            assert_eq!(Lang::of(name), Lang::Syntax(syntax), "{name}");
        }
        assert_eq!(
            Lang::of("app.jsx"),
            Lang::JavaScript,
            "tree-sitter's, not bat's"
        );
        assert_eq!(Lang::of("server.mjs"), Lang::JavaScript);
        assert_eq!(Lang::of("LICENSE"), Lang::Plain);
        assert_eq!(Lang::of("notes.txt"), Lang::Plain, "plain text is plain");
    }

    #[test]
    fn a_shebang_or_jinja_tags_say_what_a_file_is() {
        let script = "#!/usr/bin/env bash\necho hi\n";
        assert_eq!(
            Lang::detect("bin/release", script),
            Lang::Syntax("Bourne Again Shell (bash)")
        );
        let template = "<ul>\n{% for item in items %}<li>{{ item }}</li>{% endfor %}\n</ul>\n";
        assert_eq!(
            Lang::detect("templates/list.html", template),
            Lang::Syntax("HTML (Jinja2)")
        );
        assert_eq!(
            Lang::detect("app.html", "<p>{{ angular }}</p>"),
            Lang::Syntax("HTML"),
            "{{ }} alone is other templating too"
        );
        assert_eq!(Lang::detect("main.rs", script), Lang::Rust, "the name wins");
    }

    #[test]
    fn bats_syntaxes_colour_what_tree_sitter_would() {
        let md = "# Title\n\nSome `code` here.\n";
        assert_eq!(
            spans(md, Lang::Syntax("Markdown"), 0),
            [("# Title".to_string(), Kind::Keyword)],
            "a heading is one colour"
        );
        assert!(has(
            &spans(md, Lang::Syntax("Markdown"), 2),
            "`code`",
            Kind::String
        ));
        let toml = "[package]\nname = \"abc\"\nversion = 3\n";
        let toml_lang = Lang::Syntax("TOML");
        assert!(
            has(&spans(toml, toml_lang, 1), "\"abc\"", Kind::String),
            "quotes and all"
        );
        assert!(has(&spans(toml, toml_lang, 2), "3", Kind::Constant));
        let html = "<a href=\"/x\">go</a>";
        let line = spans(html, Lang::Syntax("HTML"), 0);
        assert!(has(&line, "a", Kind::Keyword), "{line:?}");
        assert!(has(&line, "href", Kind::Property));
        assert!(
            line.iter()
                .any(|(t, k)| t.contains("/x") && *k == Kind::String)
        );
        let jinja = "{% if user %}{{ user.name }}{% endif %}";
        let line = spans(jinja, Lang::Syntax("Jinja2"), 0);
        assert!(has(&line, "if", Kind::Keyword), "{line:?}");
    }

    #[test]
    fn spans_from_bats_syntaxes_stay_within_their_lines() {
        let src = "# a\n\n```rust\nfn main() {}\n```\nlast line without a newline";
        let lines: Vec<&str> = src.split('\n').collect();
        let highlighted = highlight(src, Lang::Syntax("Markdown")).unwrap();
        assert_eq!(highlighted.len(), lines.len());
        for (i, spans) in highlighted.iter().enumerate() {
            for s in spans {
                assert!(
                    s.start < s.end && s.end <= lines[i].len(),
                    "line {i}: {s:?}"
                );
            }
        }
    }

    #[test]
    fn a_file_with_a_huge_line_shows_plain() {
        let minified = format!("{{\"a\": \"{}\"}}\n", "x".repeat(6_000));
        assert!(highlight(&minified, Lang::Syntax("JSON")).is_none());
        assert!(highlight("{\"a\": 1}\n", Lang::Syntax("JSON")).is_some());
    }

    #[test]
    fn labels_are_short() {
        assert_eq!(Lang::Syntax("Markdown").label(), "markdown");
        assert_eq!(Lang::Syntax("Bourne Again Shell (bash)").label(), "bash");
        assert_eq!(Lang::Syntax("HTML (Jinja2)").label(), "jinja2");
        assert_eq!(Lang::JavaScript.label(), "js");
        assert_eq!(Lang::Rust.label(), "rust");
    }

    #[test]
    fn javascript_and_jsx_use_tree_sitter() {
        let src = "export const App = () => <Button label=\"go\" />\n// done";
        let line = spans(src, Lang::JavaScript, 0);
        assert!(has(&line, "export", Kind::Keyword));
        assert!(has(&line, "\"go\"", Kind::String));
        assert!(has(
            &spans(src, Lang::JavaScript, 1),
            "// done",
            Kind::Comment
        ));
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
