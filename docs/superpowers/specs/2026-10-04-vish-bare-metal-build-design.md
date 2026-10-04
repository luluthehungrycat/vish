# VISH Bare Metal Build Design

## Goal

Provide stable `nasm` and `elf` build targets that VIBIX integration can invoke, while preventing the NASM listing step from overwriting tracked files and enabling Rust shell integration tests.

## Current state

VIBIX invokes `make -C ../vish nasm` for the flat shell and `make -C ../vish elf` for an experimental Rust ELF at `target/x86_64-unknown-none/release/vibix`. The selected VISH Makefile has neither target. Its listing command runs NASM without an output path; NASM consequently overwrites the tracked `src/vibix/vish` file. The Rust crate has a `no_std`-compatible core but only a Linux executable entry point. The VIBIX core is assembly today.

## Design

Add `nasm` as an explicit alias for building `vish.bin`. Keep the assembly output path explicit. Make the informational listing use `/dev/null` as its output so the recipe cannot replace the tracked source-adjacent binary.

Implement a real Rust VIBIX executable for `x86_64-unknown-none`, reusing the existing parser, readline, history, and builtin core where those modules support `no_std`. Add a VIBIX-only executable entry point with the kernel syscall-backed byte I/O, required allocation setup, panic/exit behavior, and target-specific Cargo configuration. The `elf` Make target must build this Rust shell with the pinned stable toolchain and place the ELF at the exact path expected by VIBIX.

Keep the existing noninteractive Rust scheduler probe separate: add a small Rust probe binary and an `elf-probe` target for the existing scheduler-evidence test. VIBIX must use `elf-probe` for `test_vibit_rust` and the shell `elf` artifact for the new interactive shell check. Do not make either target an alias for the NASM flat binary or report a build-only placeholder as Rust integration.

Add a dedicated VIBIT/VISH Rust shell integration check that boots the Rust ELF, sends shell input, and verifies prompt, command output, exit, child reaping, and respawn. Do not reuse `test_vibit_rust` for the shell smoke: the existing test expects `OK` and scheduler-evidence markers from a Rust ELF probe, not an interactive shell. Preserve that scheduler probe test as a separate check. Preserve the current flat NASM shell target and Linux build/test behavior.

## Validation

First verify the default `make` and `make nasm` create the same flat binary without changing tracked files. Verify `make elf` produces a valid x86-64 ELF at the VIBIX expected path. Run VISH Linux unit tests. Run the existing VIBIX Rust scheduler probe independently. Run the new Rust shell integration check with scripted input and verify its serial evidence includes prompt, builtin output, a child exit/reap, and shell respawn. Run the WASI and browser WASM build checks for GVIBU after required bug and shell integration checks pass; report missing optional runtimes separately.

## Constraints

- Preserve the Linux executable and flat binary interfaces.
- Use the repository's locked dependencies and Rust stable toolchain; add no dependency unless the existing core demonstrably requires it.
- Preserve source, lockfile, and build-artifact boundaries across the four repositories.
- Keep VIBIX kernel changes out of this design unless integration evidence identifies a kernel defect.
