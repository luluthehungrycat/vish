# vish — VIBIX SHell (also: Linux SHell)

A cross-platform shell that runs on **bare-metal VIBIX** (flat binary) and **Linux** (ELF binary), sharing one core written in Rust with platform-specific I/O backends.

## Why vish?

The VIBIX kernel already has `vibix_shell.inc` — an interactive serial shell compiled into the kernel blob. vish takes the next step:

- **Standalone binary** — launched by VIBIT (PID 1 init) as a proper userspace process
- **Cross-platform** — same shell code runs on Linux for fast iteration and testing
- **Pipes and redirection** — `cmd1 | cmd2 > file`
- **Job control** — `&`, `fg`, `bg`, `jobs`
- **Environment variables** — `$PATH`, `$HOME`, `export`
- **Scripting** — `vish script.vsh` or `#!/usr/bin/vish`
- **bash-compatible subset** — long-term goal for script portability

## Architecture

```
┌─────────────────────────────┐
│     Shell Core (Rust)        │  ← parsing, expansion, job control,
│  readline, history, builtins │     command dispatch, scripting
└──────────┬──────────────────┘
           │
     ┌─────┴─────┐
     ▼           ▼
┌─────────┐ ┌─────────┐
│ Linux    │ │ VIBIX   │  ← platform I/O layer (~500 lines)
│ backend  │ │ backend │     syscalls, terminal, process mgmt
│ (ELF)    │ │ (flat)  │
└─────────┘ └─────────┘
```

## Project Ecosystem

vish is part of a family of projects:

| Project | Path | Purpose |
|---------|------|---------|
| **vibix** | `../vibix-ai-lab` | Bare-metal x86-64 kernel (Rust + NASM) — targets vish as a userspace process |
| **gvibu** | `../gvibu-ai-lab` | Unix coreutils in Rust — reference for command behavior |
| **vibit** | `../vibit` | PID 1 init system (NASM) — launches vish as the user-facing shell |
| **vish** | `.` | This project — cross-platform shell |

## Quick Start (Linux)

```bash
cargo build --release
./target/release/vish
```

## Quick Start (VIBIX)

```bash
nasm -f bin src/vibix/vish.asm -o vish.bin
# Copy vish.bin to the VIBIX kernel's userspace blob
```

## License

MIT
