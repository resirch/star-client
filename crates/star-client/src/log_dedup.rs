//! Log writer that collapses consecutive identical lines (ignoring the timestamp).
//!
//! On a terminal the repeated line is updated in place with a counter, e.g.
//! `... Is the Riot Client signed in? (x5)`. When stdout is not a terminal
//! (redirected to a file), repeats are suppressed and a single summary line is
//! written once a different line arrives.

use parking_lot::Mutex;
use std::io::{self, IsTerminal, Write};
use tracing_subscriber::fmt::MakeWriter;

const SAVE_CURSOR: &str = "\x1b7";
const RESTORE_CURSOR: &str = "\x1b8";
const CLEAR_TO_EOL: &str = "\x1b[K";

struct State {
    /// stdout supports in-place rewriting via VT escape sequences.
    live: bool,
    /// Live mode: the last line was written without its trailing newline so the
    /// saved cursor position stays valid (a newline could scroll the screen).
    line_open: bool,
    last_key: Option<String>,
    repeats: u32,
}

static STATE: Mutex<State> = parking_lot::const_mutex(State {
    live: false,
    line_open: false,
    last_key: None,
    repeats: 0,
});

/// `MakeWriter` for `tracing_subscriber::fmt().with_writer(..)`.
#[derive(Clone, Copy)]
pub struct DedupStdout;

/// Detects whether stdout can be rewritten in place and returns the writer.
/// Call before installing the subscriber; pass `is_live()` to `with_ansi`.
pub fn init() -> DedupStdout {
    STATE.lock().live = io::stdout().is_terminal() && enable_vt_processing();
    DedupStdout
}

pub fn is_live() -> bool {
    STATE.lock().live
}

/// Terminates the open line / writes a pending repeat summary. Call before exit.
pub fn finish() {
    let mut state = STATE.lock();
    let mut out = io::stdout().lock();
    if state.live {
        if state.line_open {
            let _ = out.write_all(b"\n");
            state.line_open = false;
        }
    } else {
        let _ = state.write_repeat_summary(&mut out);
    }
    state.last_key = None;
    state.repeats = 0;
    let _ = out.flush();
}

impl<'a> MakeWriter<'a> for DedupStdout {
    type Writer = EventWriter;

    fn make_writer(&'a self) -> Self::Writer {
        EventWriter {
            buf: Vec::with_capacity(256),
        }
    }
}

/// Buffers one formatted event; emitted as a whole when dropped.
pub struct EventWriter {
    buf: Vec<u8>,
}

impl Write for EventWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for EventWriter {
    fn drop(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        let text = String::from_utf8_lossy(&self.buf);
        let line = text.trim_end_matches(['\r', '\n']);
        let key = dedup_key(line);

        let mut state = STATE.lock();
        let mut out = io::stdout().lock();
        let _ = state.write_line(&mut out, line, key);
        let _ = out.flush();
    }
}

impl State {
    fn write_line(&mut self, out: &mut impl Write, line: &str, key: String) -> io::Result<()> {
        if self.last_key.as_deref() == Some(key.as_str()) {
            self.repeats += 1;
            if self.live {
                write!(out, "{RESTORE_CURSOR} (x{}){CLEAR_TO_EOL}", self.repeats)?;
            }
            return Ok(());
        }

        if self.live {
            if self.line_open {
                out.write_all(b"\n")?;
            }
            write!(out, "{line}{SAVE_CURSOR}")?;
            self.line_open = true;
        } else {
            self.write_repeat_summary(out)?;
            writeln!(out, "{line}")?;
        }

        self.last_key = Some(key);
        self.repeats = 1;
        Ok(())
    }

    fn write_repeat_summary(&mut self, out: &mut impl Write) -> io::Result<()> {
        if self.repeats > 1 {
            writeln!(out, "    (previous line repeated x{})", self.repeats)?;
        }
        self.repeats = 0;
        Ok(())
    }
}

/// The line with ANSI escapes and the leading timestamp removed.
fn dedup_key(line: &str) -> String {
    let mut plain = String::with_capacity(line.len());
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.next() == Some('[') {
                // CSI sequence: skip until the final byte (0x40..=0x7E).
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            continue;
        }
        plain.push(c);
    }
    match plain.split_once(' ') {
        Some((_timestamp, rest)) => rest.trim_start().to_owned(),
        None => plain,
    }
}

#[cfg(windows)]
fn enable_vt_processing() -> bool {
    use windows_sys::Win32::System::Console::{
        GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_VIRTUAL_TERMINAL_PROCESSING,
        STD_OUTPUT_HANDLE,
    };

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0;
        if GetConsoleMode(handle, &mut mode) == 0 {
            return false;
        }
        mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING != 0
            || SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING) != 0
    }
}

#[cfg(not(windows))]
fn enable_vt_processing() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(live: bool, lines: &[&str]) -> String {
        let mut state = State {
            live,
            line_open: false,
            last_key: None,
            repeats: 0,
        };
        let mut out = Vec::new();
        for line in lines {
            state.write_line(&mut out, line, dedup_key(line)).unwrap();
        }
        state.write_repeat_summary(&mut out).unwrap();
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn key_ignores_timestamp_and_ansi() {
        assert_eq!(
            dedup_key("\x1b[2m2026-11-14T10:22:51Z\x1b[0m \x1b[33m WARN\x1b[0m a: msg"),
            "WARN a: msg"
        );
        assert_eq!(
            dedup_key("2026-11-14T10:22:53Z  WARN a: msg"),
            dedup_key("2026-11-14T10:22:55Z  WARN a: msg")
        );
    }

    #[test]
    fn live_mode_rewrites_counter_in_place() {
        let out = run(true, &["t1 A", "t2 A", "t3 A", "t4 B"]);
        assert_eq!(
            out,
            "t1 A\x1b7\x1b8 (x2)\x1b[K\x1b8 (x3)\x1b[K\nt4 B\x1b7"
        );
    }

    #[test]
    fn plain_mode_summarizes_repeats() {
        let out = run(false, &["t1 A", "t2 A", "t3 A", "t4 B"]);
        assert_eq!(out, "t1 A\n    (previous line repeated x3)\nt4 B\n");
    }
}
