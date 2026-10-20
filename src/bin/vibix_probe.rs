#![no_std]
#![no_main]

core::arch::global_asm!(
    ".section .text.vibix_probe_syscalls,\"ax\"",
    ".global vibix_probe_write",
    ".type vibix_probe_write,@function",
    "vibix_probe_write:",
    "push rbx",
    "push rbp",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov rax, 1",
    "syscall",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbp",
    "pop rbx",
    "ret",
    ".size vibix_probe_write, .-vibix_probe_write",
);

extern "C" {
    fn vibix_probe_write(fd: usize, buffer: *const u8, len: usize) -> usize;
}

core::arch::global_asm!(
    ".section .text._start,\"ax\"",
    ".global _start",
    "_start:",
    "and rsp, -16",
    "call vibix_probe_main",
    "ud2",
);

#[no_mangle]
extern "C" fn vibix_probe_main() -> ! {
    loop {
        unsafe {
            vibix_probe_write(1, b"OK\n".as_ptr(), 3);
        }
        // Keep the process runnable across timer ticks. VIBIX syscall 5
        // currently busy-waits with interrupts masked, so it cannot yield.
        for _ in 0..5_000_000 {
            core::hint::spin_loop();
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop { core::hint::spin_loop(); }
}
