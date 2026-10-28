//! Copying to the clipboard. On this machine abc hands the text to the
//! system's tool: pbcopy on macOS, wl-copy, xclip or xsel on Linux. Over ssh,
//! where this machine's clipboard isn't the user's, or with no tool, it asks
//! the terminal with OSC 52, which some terminals (Apple's among them) ignore.

use std::io::{self, Write};
use std::process::{Command, Stdio};

/// A program that copies what it reads on stdin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tool {
    program: &'static str,
    args: &'static [&'static str],
    env: &'static [(&'static str, &'static str)],
}

const PBCOPY: Tool = Tool {
    program: "pbcopy",
    args: &[],
    // Without a UTF-8 locale pbcopy mangles anything past ASCII.
    env: &[("LC_CTYPE", "UTF-8")],
};
const WL_COPY: Tool = Tool {
    program: "wl-copy",
    args: &[],
    env: &[],
};
const XCLIP: Tool = Tool {
    program: "xclip",
    args: &["-selection", "clipboard"],
    env: &[],
};
const XSEL: Tool = Tool {
    program: "xsel",
    args: &["--clipboard", "--input"],
    env: &[],
};

/// Puts `text` on the clipboard: with the first of the system's tools that's
/// installed, or else by writing OSC 52 to `terminal`.
pub fn copy(text: &str, terminal: &mut impl Write) -> Result<(), String> {
    let set = |var: &str| std::env::var_os(var).is_some();
    for tool in tools(cfg!(target_os = "macos"), set) {
        match run(&tool, text) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("{}: {e}", tool.program)),
        }
    }
    terminal
        .write_all(osc52(text).as_bytes())
        .and_then(|()| terminal.flush())
        .map_err(|e| e.to_string())
}

/// The tools to try, in order, given whether this is macOS and which
/// environment variables are set. None over ssh.
fn tools(macos: bool, set: impl Fn(&str) -> bool) -> Vec<Tool> {
    if set("SSH_TTY") || set("SSH_CONNECTION") {
        return Vec::new();
    }
    if macos {
        return vec![PBCOPY];
    }
    let mut tools = Vec::new();
    if set("WAYLAND_DISPLAY") {
        tools.push(WL_COPY);
    }
    if set("DISPLAY") {
        tools.extend([XCLIP, XSEL]);
    }
    tools
}

/// Runs `tool` with `text` on its stdin.
fn run(tool: &Tool, text: &str) -> io::Result<()> {
    let mut child = Command::new(tool.program)
        .args(tool.args)
        .envs(tool.env.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    // Dropping stdin closes it, so the tool knows the text is all there.
    let written = child
        .stdin
        .take()
        .map_or(Ok(()), |mut stdin| stdin.write_all(text.as_bytes()));
    let status = child.wait()?;
    written?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("exited with {status}")))
    }
}

/// The escape sequence that puts `text` on the clipboard.
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

/// Standard base64, padded.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(tools: &[Tool]) -> Vec<&str> {
        tools.iter().map(|t| t.program).collect()
    }

    #[test]
    fn a_mac_copies_with_pbcopy() {
        assert_eq!(names(&tools(true, |_| false)), ["pbcopy"]);
    }

    #[test]
    fn linux_copies_with_the_tool_for_its_display() {
        assert_eq!(
            names(&tools(false, |v| v == "WAYLAND_DISPLAY")),
            ["wl-copy"]
        );
        assert_eq!(names(&tools(false, |v| v == "DISPLAY")), ["xclip", "xsel"]);
        assert!(
            tools(false, |_| false).is_empty(),
            "no display: the terminal copies"
        );
    }

    #[test]
    fn over_ssh_the_terminal_copies() {
        assert!(tools(true, |v| v == "SSH_TTY").is_empty());
        assert!(tools(false, |v| v == "SSH_CONNECTION" || v == "DISPLAY").is_empty());
    }

    #[test]
    fn running_a_tool_says_whether_it_worked() {
        let tool = |program| Tool {
            program,
            args: &[],
            env: &[],
        };
        assert!(run(&tool("cat"), "text").is_ok());
        assert!(run(&tool("false"), "text").is_err());
        let missing = run(&tool("abc-no-such-program"), "text").unwrap_err();
        assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn base64_matches_the_rfc_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (plain, encoded) in cases {
            assert_eq!(base64(plain.as_bytes()), encoded, "{plain:?}");
        }
    }

    #[test]
    fn osc52_wraps_the_text_for_the_clipboard() {
        assert_eq!(osc52("hi\n"), "\x1b]52;c;aGkK\x07");
    }
}
