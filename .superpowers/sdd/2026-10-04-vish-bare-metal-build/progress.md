# SDD ledger — plan: docs/superpowers/plans/2026-10-04-vish-bare-metal-build.md

Pre-flight: Task 2 produces the `vibix` shell ELF consumed by Task 3; Task 2 also produces the separate `vibix_probe` consumed by the existing VIBIX scheduler test. VIBIT's test path depends on Task 1's `nasm` alias only for the standard flat-shell check.
Ruling: preserve scheduler evidence by building a separate Rust probe ELF — the current `test_vibit_rust` expects `OK` and scheduler markers and cannot validate an interactive shell — cost if wrong: one extra test executable and target.
Ruling: keep VISH target outputs in the existing Cargo target tree because VIBIX Makefile resolves those exact paths — onboarding tools/targets may be outside, but the integration consumer hardcodes this artifact location — cost if wrong: generated ignored outputs can coexist with preserved prior artifacts.

Implementation and required verification complete. `pytest -q tests/test_build_targets.py` passed; `cargo +stable test --locked --lib` passed all 56 tests; `make nasm elf elf-probe` built all three targets. The separate VIBIX Rust shell lifecycle test and scheduler probe both passed. Commit `94c8885` is pushed on `codex/onboarding-fixes`; draft PR creation awaits GitHub CLI authentication.
