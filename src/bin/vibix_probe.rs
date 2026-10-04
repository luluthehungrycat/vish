#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {
        let mut result = 1usize;
        unsafe {
            core::arch::asm!("syscall", inlateout("rax") result,
                in("rdi") 1usize, in("rsi") b"OK\n".as_ptr(), in("rdx") 3usize,
                lateout("rcx") _, lateout("r11") _, options(nostack));
        }
        let mut result = 5usize;
        unsafe {
            core::arch::asm!("syscall", inlateout("rax") result,
                in("rdi") 0usize, in("rsi") 10_000_000usize,
                lateout("rcx") _, lateout("r11") _, options(nostack));
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop { core::hint::spin_loop(); }
}
