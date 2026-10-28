//! Prints every gallery state cell by cell, for the screens page, so the page
//! shows exactly what abc draws. For each state: a header line
//! `screen <screen> <state> <width> <height>`, then one line per cell, row by
//! row: `<symbol> <fg> <bg> <modifiers>`, tab-separated. Colours are `#rrggbb`,
//! or `-` for the terminal's default; modifiers are `b` (bold), `u`
//! (underlined), or `-`.
//!
//!     cargo run -q -p abc --example export_screens > target/screens.tsv
//!
//! Besides the gallery states it exports `base` states (the populated frame
//! with each pane focused) and `icon` lines (file icons in both colour sets),
//! for mockups of screens not built yet.

use abc::app::{App, Focus};
use abc::gallery::{self, Screen};
use abc::theme::Theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};

fn color(c: Color) -> String {
    match c {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Reset => "-".into(),
        other => format!("{other:?}"),
    }
}

fn dump(screen: &str, state: &str, buf: &Buffer) {
    let area = buf.area;
    println!("screen\t{screen}\t{state}\t{}\t{}", area.width, area.height);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &buf[(x, y)];
            let mut modifiers = String::new();
            if cell.modifier.contains(Modifier::BOLD) {
                modifiers.push('b');
            }
            if cell.modifier.contains(Modifier::UNDERLINED) {
                modifiers.push('u');
            }
            if modifiers.is_empty() {
                modifiers.push('-');
            }
            println!(
                "{}\t{}\t{}\t{modifiers}",
                cell.symbol(),
                color(cell.fg),
                color(cell.bg)
            );
        }
    }
}

fn render(app: &App, size: (u16, u16), theme: &Theme) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(size.0, size.1)).expect("a test terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            abc::ui::draw(frame, area, app, theme);
        })
        .expect("drawing");
    terminal.backend().buffer().clone()
}

fn main() {
    let theme = Theme::dark();
    let presets = gallery::presets(Screen::Frame);
    for preset in &presets {
        dump(
            "frame",
            preset.name,
            &render(&preset.app, preset.size.unwrap_or((120, 40)), &theme),
        );
    }
    for (name, screen) in [("explorer", Screen::Explorer), ("viewer", Screen::Viewer)] {
        for preset in gallery::presets(screen) {
            dump(name, preset.name, &render(&preset.app, (120, 40), &theme));
        }
    }
    let populated = &presets[0].app;
    for (name, focus) in [
        ("explorer", Focus::Explorer),
        ("viewer", Focus::Viewer),
        ("sidebar", Focus::Sidebar),
    ] {
        let app = App {
            focus,
            ..populated.clone()
        };
        dump("base", name, &render(&app, (120, 40), &theme));
    }
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).expect("a test terminal");
    terminal
        .draw(|frame| {
            let area = frame.area();
            abc::ui::catalog::draw(frame.buffer_mut(), area, &theme);
        })
        .expect("drawing");
    dump("icons", "system", terminal.backend().buffer());
    terminal
        .draw(|frame| {
            let area = frame.area();
            abc::ui::catalog::draw_files(frame.buffer_mut(), area, &theme);
        })
        .expect("drawing");
    dump("icons", "files", terminal.backend().buffer());

    // File icons for the page's mockups, in both colour sets:
    // `icon <name> <glyph> <dark colour> <light colour>`.
    let light = Theme::light();
    let mockups = [
        "client.ts",
        "verify.ts",
        "score.py",
        "verifier.py",
        "text.ts",
        "main.tsx",
        "dump.sql",
        "gst.md",
        "logo.png",
        "rows.rs",
        ".gitignore",
        "package.json",
        "README.md",
        "tsconfig.json",
        "verify-gst.tsx",
    ];
    let catalogue = abc::ui::catalog::FILES
        .iter()
        .flat_map(|(_, names)| names.iter().copied());
    for name in mockups.into_iter().chain(catalogue) {
        let (glyph, dark) = abc::icons::file(name, &theme);
        let (_, light) = abc::icons::file(name, &light);
        println!("icon\t{name}\t{glyph}\t{}\t{}", color(dark), color(light));
    }
}
