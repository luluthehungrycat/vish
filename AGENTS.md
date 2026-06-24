# AGENTS.md — Guidelines for AI Agents Working on vish

## Project Context

vish (VIBIX SHell) is a cross-platform shell that runs on bare-metal VIBIX and Linux.
It is part of a family of closely related projects. Understanding them is essential.

### Sibling Repositories

All paths below are relative to this repository (`vish/`).

#### `../vibix-ai-lab` — VIBIX Kernel
An x86-64 bare-metal kernel (Rust + NASM assembly). vish runs as a userspace process
on VIBIX. Key interfaces vish must conform to:

- **Syscall ABI**: `syscall` instruction (`rax` = number, `rdi/rsi/rdx` = args, return in `rax`)
- **Current syscalls**: `exit(0)`, `write(1, buf, len)`, `read(0, buf, len)`, `getpid()`, `brk(addr)`
- **Binary formats**: Flat binary (`nasm -f bin`) and ELF64
- **Memory layout**: Code at `0x2000000`, stack at `0x2002000` (4 KiB), heap at `0x2010000`
- **Process model**: Currently single-process (PID 1). No `fork`/`exec` yet — designing vish must anticipate these.
- **Serial console**: stdout = COM1 @ 115200 8N1. stdin = PS/2 keyboard buffer (non-blocking read).

Reference files to consult (read-only):
- `SYSCALL.md` — full syscall ABI specification
- `userspace/vibix_shell.inc` — existing interactive shell (897 lines, NASM)
- `userspace/vibix_core.inc` — I/O helpers (write_str, read_char, etc.)
- `kernel_rust/src/syscall.rs` — kernel-side syscall dispatch

#### `../gvibu-ai-lab` — GVIBU Coreutils
A Rust project implementing 58 Unix commands (echo, cat, ls, find, grep, etc.).
Relevant as a reference for command behavior and flags. Not directly linked to vish
at the code level.

#### `../vibit` — VIBIT Init System
A NASM flat binary that serves as PID 1 on VIBIX. vish will be launched by VIBIT
once the kernel supports multiprocessing. Currently VIBIT is a minimal init
(~300 lines NASM) that runs test cases and exits.

### Repository Boundaries (Critical)

**You are working in `vish/`. The sibling repos are read-only. You MUST NOT:**

1. **Edit, modify, or delete** any files in `../vibix-ai-lab/`, `../gvibu-ai-lab/`, or `../vibit/`.
2. **Create new files** in those sibling directories.
3. **Run destructive commands** (git push, git commit, rm, mv, etc.) in those directories.
4. **Copy files** from vish into any sibling repo.

**If you believe a change in a sibling repo is necessary**, do not make it yourself.
Instead, ask the user for explicit permission, describing exactly what needs to change,
which file, and why. Wait for a go-ahead.

### Reading Sibling Repos

You may freely **read** files in sibling repos to understand:
- The VIBIX syscall ABI (for the VIBIX backend)
- The existing shell code for reference (`vibix_shell.inc`)
- The GVIBU command implementations for behavior parity
- The VIBIT interface (how it will launch vish)

## Development Workflow

### Code Organization

```
src/
├── core/           # Platform-independent shell core (Rust)
│   ├── parse.rs    # Command line parsing (quotes, escapes, pipes, redirections)
│   ├── expand.rs   # Variable expansion ($VAR, $(), etc.)
│   ├── job.rs      # Job control (foreground, background, process groups)
│   ├── builtins/   # Built-in commands (cd, export, echo, etc.)
│   ├── readline.rs # Line editing with history
│   └── exec.rs     # Command execution and dispatch
├── linux/          # Linux backend (Rust)
│   ├── sys.rs      # Linux syscall wrappers
│   └── main.rs     # Linux entry point
└── vibix/          # VIBIX backend
    ├── sys.asm     # VIBIX syscall wrappers (NASM)
    └── main.asm    # VIBIX flat binary entry point (NASM)
```

### First Principles

- Write core logic in Rust first, test on Linux, then adapt the VIBIX backend
- Use `core` library (`#![no_std]`) for VIBIX, `std` for Linux (conditional compilation)
- The VIBIX backend is NASM only where necessary (entry point, syscall stubs)
- Prefer simple, correct implementations over clever or fast ones

### Testing

- Linux: `cargo test` for unit tests, integration tests in `tests/`
- Linux: manual testing via `./target/release/vish`
- VIBIX: build flat binary → copy to `vibix-ai-lab/userspace/` (ask permission first)
- VIBIX: run via `make run` in the vibix repo (QEMU)

### Build Commands

```bash
# Linux
cargo build
cargo run
cargo test

# VIBIX (flat binary)
nasm -f bin src/vibix/main.asm -o vish.bin
```
