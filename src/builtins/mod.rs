//! Built-in command dispatcher.
//!
//! Each builtin is a module with a function matching the signature:
//!   `fn(args: &[String], env: &mut Vec<String>, writer: &mut dyn WriteStr) -> i32`

use alloc::vec::Vec;
use alloc::string::String;
use crate::io::WriteStr;

pub enum BuiltinResult {
    /// Command handled normally (exit code).
    Handled(i32),
    /// Exit the shell with this code.
    Exit(i32),
    /// Not a builtin.
    NotFound,
}

mod echo;
mod cat;
mod printenv;
mod clear;
mod true_false;
mod exit_builtin;
mod help;
pub(crate) mod history_builtin;
mod cd;

/// Look up and execute a builtin command.
///
/// Returns `NotFound` if `name` is not a known builtin.
pub fn dispatch(
    name: &str,
    args: &[String],
    env: &mut Vec<String>,
    writer: &mut dyn WriteStr,
) -> BuiltinResult {
    match name {
        "echo" => BuiltinResult::Handled(echo::echo(args, writer)),
        "cat" => BuiltinResult::Handled(cat::cat(args, writer)),
        "printenv" => BuiltinResult::Handled(printenv::printenv(args, env, writer)),
        "clear" => BuiltinResult::Handled(clear::clear(writer)),
        "true" => BuiltinResult::Handled(true_false::true_cmd()),
        "false" => BuiltinResult::Handled(true_false::false_cmd()),
        "exit" => exit_builtin::exit(args, writer),
        "help" => BuiltinResult::Handled(help::help(writer)),
        "history" => BuiltinResult::Handled(history_builtin::history(writer)),
        "cd" => BuiltinResult::Handled(cd::cd(args, env, writer)),
        _ => BuiltinResult::NotFound,
    }
}

/// Names of all builtins, in display order.
pub const BUILTIN_NAMES: &[&str] = &[
    "cat",
    "cd",
    "clear",
    "echo",
    "exit",
    "false",
    "help",
    "history",
    "printenv",
    "true",
];
