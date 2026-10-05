// vish core library — platform-independent shell logic
//
// This crate is `#![no_std]`-compatible for the VIBIX bare-metal backend.
// On Linux, the `std` feature (default) provides std integration.
// In either mode, heap types come from `extern crate alloc`.

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

pub mod io;
pub mod parse;
pub mod readline;
pub mod exec;
pub mod builtins;

#[cfg(test)]
#[path = "vibix/allocator.rs"]
mod allocator_tests;

#[cfg(test)]
#[path = "vibix/runtime.rs"]
mod runtime_tests;
