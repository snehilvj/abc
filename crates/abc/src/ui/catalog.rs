//! `abc gallery icons`: every icon abc uses, with what it means, drawn with a
//! 1-cell and a 2-cell gap side by side, plus real sidebar rows in each. It is
//! the reference for the icon system and the way to choose `icon_gap` for a
//! terminal.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};
use ratatui::text::{Line, Span};

use super::label::{badge, labelled};
use super::sidebar;
use crate::app::SidebarStyle;
use crate::fake;
use crate::icons;
use crate::theme::Theme;

/// What each icon means, as its label in the catalogue, and its colour role.
fn vocabulary(theme: &Theme) -> Vec<(&'static str, &'static str, Option<Color>)> {
    vec![
        (icons::REPO, "repository", None),
        (icons::HOME, "main checkout", None),
        (icons::CURRENT, "current worktree", None),
        (icons::BRANCH, "branch", None),
        (icons::COMMIT, "detached HEAD", None),
        (icons::PR_OPEN, "#142 open", Some(theme.pr_open)),
        (icons::PR_DRAFT, "#88 draft", Some(theme.pr_draft)),
        (icons::PR_MERGED, "#141 merged", Some(theme.pr_merged)),
        (icons::PR_CLOSED, "#97 closed", Some(theme.pr_closed)),
        (icons::CHECKS_PASSING, "checks passing", Some(theme.ok)),
        (icons::CHECKS_FAILING, "checks failing", Some(theme.error)),
        (icons::CHECKS_PENDING, "checks running", Some(theme.warning)),
        (icons::CHANGED, "3 changed files", Some(theme.git_changed)),
        (icons::AHEAD, "2 ahead", Some(theme.git_ahead)),
        (icons::BEHIND, "1 behind", Some(theme.git_behind)),
        (icons::LOCK, "locked", None),
        (icons::WARNING, "missing", Some(theme.warning)),
        (icons::SUCCESS, "added privue-agents", Some(theme.ok)),
        (icons::FAILURE, "abcd did not start", Some(theme.error)),
        (icons::INFO, "abcd restarted", Some(theme.accent)),
        (icons::SYNC, "starting abcd…", Some(theme.accent)),
        (icons::DISCONNECTED, "lost abcd", Some(theme.error)),
    ]
}

pub fn draw(buf: &mut Buffer, area: Rect, theme: &Theme) {
    let half = area.width / 2;
    let top = area.y + 1;
    buf.set_string(
        area.x + 2,
        top,
        "ICONS AND LABELS",
        theme.accent().add_modifier(Modifier::BOLD),
    );
    buf.set_string(
        area.x + 2,
        top + 1,
        "Pick the column where every icon sits clear of its label, then press q.",
        theme.dim(),
    );
    for (i, (gap, name)) in [
        (1, "A  one cell after each icon"),
        (2, "B  two cells after each icon"),
    ]
    .into_iter()
    .enumerate()
    {
        let theme = Theme {
            icon_gap: gap,
            ..theme.clone()
        };
        let x = area.x + 2 + i as u16 * half;
        column(
            buf,
            Rect::new(
                x,
                top + 3,
                half.saturating_sub(4),
                area.height.saturating_sub(5),
            ),
            name,
            &theme,
        );
    }
}

fn column(buf: &mut Buffer, area: Rect, name: &str, theme: &Theme) {
    let mut y = area.y;
    buf.set_string(area.x, y, name, theme.bold());
    y += 2;
    let repos = fake::repos_with_variants();
    let options = sidebar::Options {
        style: SidebarStyle::Detailed,
        repo_gap: 1,
        worktree_gap: 0,
        width: 38,
    };
    for row in sidebar::rows(&repos[..1], Some(&fake::current()), &options, theme)
        .iter()
        .take(9)
    {
        buf.set_line(area.x.saturating_sub(1), y, &row.line, 38);
        y += 1;
    }
    y += 1;
    for (icon, meaning, color) in vocabulary(theme) {
        if y >= area.bottom() {
            break;
        }
        let spans = match color {
            Some(color) => badge(icon, meaning, color, theme),
            None => labelled(icon, theme.accent(), meaning, theme.text(), theme),
        };
        let mut line = vec![Span::raw("  ")];
        line.extend(spans);
        buf.set_line(area.x, y, &Line::from(line), area.width);
        y += 1;
    }
}

/// File types as the explorer and switcher will show them, grouped.
pub const FILES: &[(&str, &[&str])] = &[
    (
        "Code",
        &[
            "main.rs",
            "index.ts",
            "App.tsx",
            "index.js",
            "Button.jsx",
            "app.py",
            "main.go",
            "app.rb",
            "Main.java",
            "Main.kt",
            "App.swift",
            "main.c",
            "main.cpp",
            "header.h",
            "Program.cs",
            "script.sh",
            "init.lua",
            "query.sql",
        ],
    ),
    (
        "Web",
        &[
            "index.html",
            "styles.css",
            "theme.scss",
            "icon.svg",
            "vite.config.ts",
            ".eslintrc.json",
            ".prettierrc",
        ],
    ),
    (
        "Config and data",
        &[
            "Cargo.toml",
            "Cargo.lock",
            "package.json",
            "package-lock.json",
            "tsconfig.json",
            "pyproject.toml",
            "requirements.txt",
            "go.mod",
            "Gemfile",
            "config.yaml",
            "settings.toml",
            "data.json",
            "data.csv",
            "feed.xml",
            ".env",
        ],
    ),
    (
        "Repo and build",
        &[
            ".gitignore",
            ".gitattributes",
            "Dockerfile",
            "docker-compose.yml",
            "Makefile",
            "justfile",
            "main.tf",
            "LICENSE",
        ],
    ),
    (
        "Docs and media",
        &[
            "README.md",
            "CHANGELOG.md",
            "notes.txt",
            "guide.pdf",
            "logo.png",
            "photo.jpg",
            "demo.gif",
            "video.mp4",
            "archive.zip",
            "backup.tar.gz",
        ],
    ),
    ("Unknown type", &["data.xyz"]),
];

/// `abc gallery icons --state files`: every common file type with its icon and colour.
pub fn draw_files(buf: &mut Buffer, area: Rect, theme: &Theme) {
    let top = area.y + 1;
    buf.set_string(
        area.x + 2,
        top,
        "FILE ICONS",
        theme.accent().add_modifier(Modifier::BOLD),
    );
    let dark = if theme.dark { "dark" } else { "light" };
    buf.set_string(
        area.x + 2,
        top + 1,
        format!("From devicons, in its {dark} colour set. Folders use the theme's folder colour."),
        theme.dim(),
    );
    let column_width = area.width.saturating_sub(4) / 3;
    let (mut col, mut y) = (0u16, top + 3);
    let bottom = area.bottom().saturating_sub(1);
    let mut put = |spans: Vec<Span>, col: &mut u16, y: &mut u16| {
        if *y >= bottom {
            *col += 1;
            *y = top + 3;
        }
        let x = area.x + 2 + *col * column_width;
        buf.set_line(x, *y, &Line::from(spans), column_width);
        *y += 1;
    };
    let folder = theme.style(theme.folder);
    put(
        vec![Span::styled("Folders", theme.bold())],
        &mut col,
        &mut y,
    );
    put(
        labelled(
            icons::FOLDER,
            folder,
            "src",
            folder.add_modifier(Modifier::BOLD),
            theme,
        ),
        &mut col,
        &mut y,
    );
    put(
        labelled(
            icons::FOLDER_OPEN,
            folder,
            "src (open)",
            folder.add_modifier(Modifier::BOLD),
            theme,
        ),
        &mut col,
        &mut y,
    );
    y += 1;
    for (group, names) in FILES {
        if y + 2 >= bottom {
            col += 1;
            y = top + 3;
        }
        put(vec![Span::styled(*group, theme.bold())], &mut col, &mut y);
        for name in *names {
            let (icon, color) = icons::file(name, theme);
            put(
                labelled(
                    icon.to_string(),
                    theme.style(color),
                    *name,
                    theme.text(),
                    theme,
                ),
                &mut col,
                &mut y,
            );
        }
        y += 1;
    }
}
