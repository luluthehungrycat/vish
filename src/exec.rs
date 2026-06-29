//! Command execution and dispatch.
//!
//! Takes a parsed `Pipeline`, dispatches builtins, and handles
//! redirections and piping (Phase 2+).

use alloc::vec::Vec;
use alloc::string::String;
use crate::parse::Pipeline;
use crate::io::WriteStr;
use crate::builtins::{self, BuiltinResult};
use crate::readline::History;

/// Result of executing a pipeline.
pub struct ExecResult {
    pub exit_code: i32,
    pub should_exit: bool,
}

/// Execute a parsed pipeline.
///
/// For Phase 2, this handles single commands only (no piping).
/// Pipe and redirect execution will be added when the Linux backend
/// provides process spawning and file descriptor plumbing.
pub fn execute(
    pipeline: &Pipeline,
    env: &mut Vec<String>,
    history: &History,
    writer: &mut dyn WriteStr,
) -> ExecResult {
    if pipeline.commands.is_empty() {
        return ExecResult {
            exit_code: 0,
            should_exit: false,
        };
    }

    // For now (Phase 2), execute only the first command.
    // Pipe handling will come when we have process management.
    let cmd = &pipeline.commands[0];

    if cmd.argv.is_empty() {
        // Empty command (blank line)
        return ExecResult {
            exit_code: 0,
            should_exit: false,
        };
    }

    let name = &cmd.argv[0];
    let args = &cmd.argv[1..];

    // Special handling for "history": pass the History struct
    if name == "history" {
        let code = builtins::history_builtin::show_history(history, writer);
        return ExecResult {
            exit_code: code,
            should_exit: false,
        };
    }

    // Handle redirections on stdout
    let _redirect_result = handle_redirects(cmd, writer);

    match builtins::dispatch(name, args, env, writer) {
        BuiltinResult::Handled(code) => ExecResult {
            exit_code: code,
            should_exit: false,
        },
        BuiltinResult::Exit(code) => ExecResult {
            exit_code: code,
            should_exit: true,
        },
        BuiltinResult::NotFound => {
            writer.write_str(name);
            writer.write_str(": command not found\r\n");
            ExecResult {
                exit_code: 127,
                should_exit: false,
            }
        }
    }
}

/// Process redirections for a command.
///
/// For Phase 2, this is a placeholder. File descriptor redirections
/// require process spawning (fork/exec or VIBIX equivalent) to work
/// correctly, because redirections affect the child process's fd table.
///
/// On Linux this will use `std::process::Command` with `.stdout()`, `.stdin()`, etc.
/// On VIBIX this will involve argument passing to commands.
fn handle_redirects(
    _cmd: &crate::parse::SimpleCommand,
    _writer: &mut dyn WriteStr,
) -> Result<(), ()> {
    // Phase 2: no-op. Redirections will be implemented when we
    // have process spawning.
    //
    // The redirects list is parsed and stored, but not yet acted upon.
    // When we implement fork/exec on Linux, each redirect will be
    // translated to a std::process::Stdio configuration.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::StringWriter;
    use crate::parse::parse_line;

    #[test]
    fn test_echo() {
        let mut w = StringWriter::new();
        let mut env = Vec::new();
        let h = History::new(4);
        let pipeline = parse_line("echo hello world").unwrap();
        let result = execute(&pipeline, &mut env, &h, &mut w);
        assert_eq!(result.exit_code, 0);
        assert!(!result.should_exit);
        assert_eq!(w.buf, "hello world\r\n");
    }

    #[test]
    fn test_true() {
        let mut w = StringWriter::new();
        let mut env = Vec::new();
        let h = History::new(4);
        let pipeline = parse_line("true").unwrap();
        let result = execute(&pipeline, &mut env, &h, &mut w);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_false() {
        let mut w = StringWriter::new();
        let mut env = Vec::new();
        let h = History::new(4);
        let pipeline = parse_line("false").unwrap();
        let result = execute(&pipeline, &mut env, &h, &mut w);
        assert_eq!(result.exit_code, 1);
    }

    #[test]
    fn test_unknown_command() {
        let mut w = StringWriter::new();
        let mut env = Vec::new();
        let h = History::new(4);
        let pipeline = parse_line("nonexistent_cmd_xyz").unwrap();
        let result = execute(&pipeline, &mut env, &h, &mut w);
        assert_eq!(result.exit_code, 127);
        assert!(w.buf.contains("command not found"));
    }

    #[test]
    fn test_empty() {
        let mut w = StringWriter::new();
        let mut env = Vec::new();
        let h = History::new(4);
        let pipeline = parse_line("").unwrap();
        let result = execute(&pipeline, &mut env, &h, &mut w);
        assert_eq!(result.exit_code, 0);
    }

    #[test]
    fn test_exit() {
        let mut w = StringWriter::new();
        let mut env = Vec::new();
        let h = History::new(4);
        let pipeline = parse_line("exit 42").unwrap();
        let result = execute(&pipeline, &mut env, &h, &mut w);
        assert!(result.should_exit);
        assert_eq!(result.exit_code, 42);
    }
}
