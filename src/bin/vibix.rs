#![no_std]
#![no_main]

extern crate alloc;

#[path = "../vibix/allocator.rs"]
mod allocator;
#[path = "../vibix/runtime.rs"]
mod runtime;

use alloc::string::String;
use alloc::vec::Vec;
use vish::exec;
use vish::io::WriteStr;
use vish::parse;
use vish::readline::{self, History};

#[global_allocator]
static ALLOCATOR: allocator::BrkAllocator = allocator::BrkAllocator;

core::arch::global_asm!(
    ".section .text._start,\"ax\"",
    ".global _start",
    "_start:",
    "and rsp, -16",
    "call vibix_main",
    "ud2",
);

#[no_mangle]
extern "C" fn vibix_main() -> ! {
    let mut reader = runtime::VibixReader;
    let mut writer = runtime::VibixWriter;
    writer.write_str("vish -- VIBIX SHell\r\n\r\n");
    let mut history = History::new(4);
    let mut env: Vec<String> = Vec::new();

    loop {
        writer.write_str(readline::PROMPT);
        let line = match readline::read_line(&mut reader, &mut writer, &mut history) {
            Ok(line) => line,
            Err(()) => runtime::syscall_exit(0),
        };
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }
        history.add(line.clone());
        let pipeline = match parse::parse_line(trimmed) {
            Ok(pipeline) => pipeline,
            Err(error) => {
                writer.write_str("parse error: ");
                writer.write_str(&error.message);
                writer.write_str("\r\n");
                continue;
            }
        };
        let result = exec::execute(&pipeline, &mut env, &history, &mut writer);
        if result.should_exit { runtime::syscall_exit(result.exit_code); }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    runtime::syscall_exit(101)
}
