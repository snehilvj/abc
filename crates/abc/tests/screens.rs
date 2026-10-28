//! Snapshot tests: every gallery state of every screen, drawn by the real
//! renderer at 120x40 (or the state's own size) in the dark theme, stored as
//! plain text so each snapshot reads like the screen's design.

use abc::gallery::{self, Screen};
use abc::theme::Theme;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

/// The buffer as lines of text, trailing spaces kept so widths stay visible.
fn text(buf: &Buffer) -> String {
    let area = buf.area;
    (area.top()..area.bottom())
        .map(|y| {
            (area.left()..area.right())
                .map(|x| buf[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn draw(width: u16, height: u16, render: impl FnOnce(&mut ratatui::Frame)) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(render).unwrap();
    text(terminal.backend().buffer())
}

#[test]
fn frame() {
    let theme = Theme::dark();
    for preset in gallery::presets(Screen::Frame) {
        let (width, height) = preset.size.unwrap_or((120, 40));
        let screen = draw(width, height, |frame| {
            let area = frame.area();
            abc::ui::draw(frame, area, &preset.app, &theme);
        });
        insta::assert_snapshot!(format!("frame-{}", preset.name), screen);
    }
}

#[test]
fn explorer() {
    let theme = Theme::dark();
    for preset in gallery::presets(Screen::Explorer) {
        let screen = draw(120, 40, |frame| {
            let area = frame.area();
            abc::ui::draw(frame, area, &preset.app, &theme);
        });
        insta::assert_snapshot!(format!("explorer-{}", preset.name), screen);
    }
}

#[test]
fn viewer() {
    let theme = Theme::dark();
    for preset in gallery::presets(Screen::Viewer) {
        let screen = draw(120, 40, |frame| {
            let area = frame.area();
            abc::ui::draw(frame, area, &preset.app, &theme);
        });
        insta::assert_snapshot!(format!("viewer-{}", preset.name), screen);
    }
}

#[test]
fn switcher() {
    let theme = Theme::dark();
    for preset in gallery::presets(Screen::Switcher) {
        let screen = draw(120, 40, |frame| {
            let area = frame.area();
            abc::ui::draw(frame, area, &preset.app, &theme);
        });
        insta::assert_snapshot!(format!("switcher-{}", preset.name), screen);
    }
}

#[test]
fn file_icons() {
    let screen = draw(120, 40, |frame| {
        let area = frame.area();
        abc::ui::catalog::draw_files(frame.buffer_mut(), area, &Theme::dark());
    });
    insta::assert_snapshot!("icons-files", screen);
}

#[test]
fn icons() {
    let screen = draw(120, 40, |frame| {
        let area = frame.area();
        abc::ui::catalog::draw(frame.buffer_mut(), area, &Theme::dark());
    });
    insta::assert_snapshot!("icons", screen);
}
