// vish — Linux binary entry point.
//
// A cross-platform shell that runs on Linux (std) and bare-metal VIBIX.
// This is the Linux backend, using std for terminal I/O and process
// management.

use std::io::{self, Read, Write};
use std::env;

use vish::io::{ReadChar, WriteStr};
use vish::parse;
use vish::readline::{self, History};
use vish::exec;

mod linux;

// ── I/O implementations ──────────────────────────────────────────────────

/// Reads characters from Linux stdin (uses raw mode).
struct StdinReader;

impl ReadChar for StdinReader {
    fn read_char(&mut self) -> Option<u8> {
        let mut buf = [0u8; 1];
        match io::stdin().read(&mut buf) {
            Ok(0) => None,   // EOF
            Ok(_) => Some(buf[0]),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                // EINTR — retry
                self.read_char()
            }
            Err(_) => None,
        }
    }
}

/// Writes to Linux stdout.
struct StdoutWriter;

impl WriteStr for StdoutWriter {
    fn write_str(&mut self, s: &str) {
        let mut stdout = io::stdout();
        let _ = stdout.write(s.as_bytes());
        let _ = stdout.flush();
    }

    fn write_char(&mut self, c: u8) {
        let mut stdout = io::stdout();
        let _ = stdout.write(&[c]);
        let _ = stdout.flush();
    }

    fn flush(&mut self) {
        let _ = io::stdout().flush();
    }
}

// ── Entry point ──────────────────────────────────────────────────────────

fn main() {
    // Enter raw mode for character-by-character input.
    // The terminal is automatically restored when `_raw` is dropped
    // at the end of main (or on panic).
    let _raw = match linux::terminal::RawMode::enter() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("vish: failed to set raw mode: {}", e);
            std::process::exit(1);
        }
    };

    let mut reader = StdinReader;
    let mut writer = StdoutWriter;
    let mut history = History::new(4);
    let mut env: Vec<String> = env::vars()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();

    // Welcome banner (quiet for now)
    // Main REPL loop
    loop {
        writer.write_str(readline::PROMPT);

        let line = match readline::read_line(&mut reader, &mut writer, &mut history) {
            Ok(line) => line,
            Err(()) => {
                // EOF / Ctrl+D on empty line
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Record in history BEFORE parsing (so partial input is saved too)
        history.add(line.clone());

        let pipeline = match parse::parse_line(trimmed) {
            Ok(p) => p,
            Err(e) => {
                let msg = format!("parse error: {}\r\n", e.message);
                writer.write_str(&msg);
                continue;
            }
        };

        // Execute (the "eval" part of REPL)
        let result = exec::execute(&pipeline, &mut env, &history, &mut writer);

        if result.should_exit {
            // Exit the shell
            std::process::exit(result.exit_code);
        }
    }
}
