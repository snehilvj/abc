//! What abc asks of the terminal while it runs, beyond ratatui's raw mode and
//! full screen: pasted text in one piece; the mouse's clicks and wheel, which
//! abc drops, so the wheel scrolls nothing (not the terminal's history, nor
//! abc, which many terminals would send it to as the up and down keys); and no
//! line wrapping, so a row drawn wider than abc thinks (a terminal that draws
//! an icon two cells wide, say) can't wrap off the last row and scroll the
//! whole screen up. All of it is undone however abc leaves, after a panic too.

use std::io::{Write, stdout};
use std::sync::atomic::{AtomicBool, Ordering};

use ratatui::crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{DisableLineWrap, EnableLineWrap};

/// Clicks and the wheel (1000), reported in SGR form (1006). Not the
/// crossterm default, which also reports every move of the mouse.
const MOUSE_ON: &str = "\x1b[?1000h\x1b[?1006h";
const MOUSE_OFF: &str = "\x1b[?1006l\x1b[?1000l";

static MOUSE: AtomicBool = AtomicBool::new(false);

/// Takes pastes, and the mouse if `mouse`, until [`release`].
pub fn take(mouse: bool) {
    let _ = execute!(stdout(), EnableBracketedPaste, DisableLineWrap);
    set_mouse(mouse);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        release();
        previous(info);
    }));
}

/// Takes the mouse from the terminal, or gives it back.
pub fn set_mouse(on: bool) {
    if MOUSE.swap(on, Ordering::SeqCst) != on {
        let mut out = stdout();
        let _ = out.write_all(if on { MOUSE_ON } else { MOUSE_OFF }.as_bytes());
        let _ = out.flush();
    }
}

/// Gives the terminal back what [`take`] took.
pub fn release() {
    set_mouse(false);
    let _ = execute!(stdout(), DisableBracketedPaste, EnableLineWrap);
}
