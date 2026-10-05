# VISH Bare Metal Build Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add safe NASM and Rust ELF build targets and validate the Rust shell under VIBIT/VIBIX.

**Architecture:** Make the flat binary target explicit and make NASM's listing output harmless. Add a VIBIX-only no-std binary that reuses VISH's parser/readline/builtin core and implements syscall I/O plus allocation. Keep the scheduler probe and interactive shell integration checks separate.

**Tech Stack:** Rust stable, `x86_64-unknown-none`, NASM, Make, Python, VIBIX QEMU TCG.

**Spec:** `docs/superpowers/specs/2026-10-04-vish-bare-metal-build-design.md`

## Global Constraints

- Preserve Linux Cargo build/test behavior and the existing flat NASM binary.
- Do not let build recipes overwrite `src/vibix/vish` or other tracked artifacts.
- Build `elf` at `target/x86_64-unknown-none/release/vibix`, as VIBIX expects.
- Reuse the no-std core; do not add dependencies unless target compilation proves one necessary.
- Leave VIBIX kernel source unchanged unless QEMU evidence demonstrates a kernel defect; update its changelog for harness/Makefile changes.

## Review Focus

- NASM informational listing has an explicit `/dev/null` output and preserves tracked source bytes.
- The ELF target compiles the Rust shell entry, not the NASM binary under an ELF filename.
- The allocator remains usable across repeated REPL iterations and is bounded by VIBIX `brk` failures.
- Shell output works over the VIBIX `read`/`write` syscall ABI and input reaches the shared line editor.
- The Rust shell test does not replace the separate `OK`/scheduler-evidence probe.

---

### Task 1: Safe NASM target

**Files:**
- Modify: `Makefile`
- Create: `tests/test_build_targets.py`

**Interfaces:**
- Produces: `make nasm` builds the existing flat `vish.bin`; default `make` remains equivalent.

- [x] **Step 1: Write `tests/test_build_targets.py`.** Add a test that runs `make nasm VISH_BIN=<temporary path>`, checks the output exists, and confirms bytes in tracked `src/vibix/vish` are unchanged. Add a second assertion that default `make` and `make nasm` produce identical flat binaries when given separate temporary output paths.
- [x] **Step 2: Run `python3 tests/test_build_targets.py` to verify RED.**

Expected: FAIL because the `nasm` target does not exist.

- [x] **Step 3: Add an overridable output variable and `nasm` alias.** Make default and alias depend on the same flat binary rule. Change the informational NASM listing invocation to specify `-o /dev/null` while keeping its listing directed to stdout.
- [x] **Step 4: Rerun `python3 tests/test_build_targets.py` to verify GREEN.**

Expected: both targets produce identical binaries and the tracked source bytes remain unchanged.

- [ ] **Step 5: Commit the Makefile change.**

### Task 2: Rust VIBIX ELF executable

**Files:**
- Create: `src/bin/vibix.rs`
- Create: `src/bin/vibix_probe.rs`
- Create: `src/vibix/runtime.rs`
- Create: `src/vibix/allocator.rs`
- Modify: `Cargo.toml` only if a target-specific binary declaration is required; do not change `Cargo.lock` unless a dependency is proven necessary.

**Interfaces:**
- Produces: `make elf` builds the no-std shell at `target/x86_64-unknown-none/release/vibix`; `make elf-probe` builds the independent scheduler probe at `target/x86_64-unknown-none/release/vibix_probe`.
- Runtime implements VISH's `ReadChar` and `WriteStr` using VIBIX syscall 2 (read fd 0), syscall 1 (write fd 1), and syscall 0 (exit).

- [x] **Step 1: Add a target build check and verify RED.** Invoke `cargo +stable build --locked --no-default-features --bin vibix --target x86_64-unknown-none --release`.

Expected before implementation: Cargo reports that binary target `vibix` does not exist.

- [x] **Step 2: Write allocator regression tests before implementation.** In `src/vibix/allocator.rs`, specify host-testable cases for aligned allocations, reuse after deallocation, adjacent-block coalescing, and failed growth. Run the named tests.

Expected: tests fail to compile because the allocator type and operations are not defined.

- [x] **Step 3: Add the VIBIX no-std executable entry and runtime.** Initialize the global allocator using VIBIX `brk` syscall 4; implement allocation and deallocation so repeated command parsing does not monotonically leak every REPL allocation. Add the `_start` entry, panic/alloc failure handling, syscall-backed input/output traits, and a REPL loop that reuses the existing parser, history, and builtin dispatch.
- [x] **Step 4: Run allocator unit tests and `cargo test --locked` to verify GREEN.** Cover allocator reuse/coalescing and host-testable syscall error/EOF translation; keep target-only syscall assembly out of host tests.

Expected: allocator tests and all Linux tests pass.

- [x] **Step 5: Add separate Rust shell and scheduler-probe binaries.** The shell entry emits the existing `vish -- VIBIX SHell` banner and shared prompt; the probe emits the `OK` marker and remains schedulable long enough for existing VIBIX evidence checks.
- [x] **Step 6: Add the `elf` and `elf-probe` Make targets.** Both use stable Cargo, `x86_64-unknown-none`, release profile, and `--no-default-features`; preserve the exact output names expected by VIBIX.
- [x] **Step 7: Build and inspect both artifacts.** Run `make elf elf-probe`; inspect both with `file`/`readelf`.

Expected: distinct valid x86-64 ET_EXEC ELFs exist at `target/x86_64-unknown-none/release/vibix` and `target/x86_64-unknown-none/release/vibix_probe`.

- [ ] **Step 8: Commit the Rust executables and build targets.**

### Task 3: Rust shell integration under VIBIT/VIBIX

**Files:**
- Modify: `/workspace/vibix/Makefile`
- Modify: `/workspace/vibix/test_kernel.py`
- Modify: `/workspace/vibix/CHANGELOG.md`

**Interfaces:**
- Consumes: `make -C ../vish elf` artifact at `../vish/target/x86_64-unknown-none/release/vibix`.
- Produces: a separate `make test_vibit_rust_shell` target; existing `test_vibit_rust` scheduler-probe semantics stay unchanged.

- [x] **Step 1: Add a focused integration assertion first.** Add a VIBIX test entry that runs the existing bounded VIBIT/vish serial interaction against a staged Rust ELF and asserts shell prompt, `help` output, exit/reap, respawn, and the next prompt.
- [x] **Step 2: Run the new test to verify RED.** Build/stage the current prerequisites and invoke the new test.

Expected before implementation: the Rust VIBIX ELF target or Rust shell integration path is missing; no false PASS from the unrelated scheduler probe.

- [x] **Step 3: Add a fixture-safe staging helper and Make target.** Build VIBIT and the Rust shell ELF, replace `/bin/vish` only in the generated initramfs fixture, rebuild using `VIBIX_STAGED_INITRAMFS=1`, invoke the new shell test, and restore the default initramfs in a `finally`/trap path. Point existing `test_vibit_rust` staging at `vibix_probe` and keep its scheduler-probe assertions.
- [x] **Step 4: Run Rust shell integration to verify GREEN.** Use QEMU TCG and scripted serial input.

Expected: Rust VISH banner and prompt, builtin output, numeric child reaping, shell respawn, and repeated prompt; no exception or panic.

- [x] **Step 5: Run the existing Rust ELF scheduler probe separately.** Invoke `test_vibit_rust` against its intended probe fixture.

Expected: existing `OK` marker and scheduler evidence checks pass independently of the interactive shell test.

- [x] **Step 6: Restore and validate standard flat-shell integration.** Rebuild the normal VIBIT flat initramfs and run `test_vibit`.

Expected: existing NASM shell/VIBIT interaction still passes.

- [ ] **Step 7: Update VIBIX changelog and commit the harness/Makefile changes.** Record all test outcomes.
