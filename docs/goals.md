# vish Goals & Roadmap

## Mission Statement

Build a minimal, correct, cross-platform shell that runs on both Linux and bare-metal VIBIX. vish should be pleasant to use day-to-day on VIBIX while also being a capable Linux shell for development and scripting. Long-term, vish aims to be a bash-compatible subset interpreter.

## Core Design Principles

1. **Correctness first** — edge cases in quoting, expansion, and job control are where shells fail. Test exhaustively on Linux, deploy confidently on VIBIX.
2. **Shared core, swappable backend** — all parsing, expansion, and job control logic is platform-independent. Only the I/O syscall layer differs between Linux and VIBIX.
3. **No bloat** — vish is a shell, not a kitchen sink. No GUI, no web server, no package manager. Every feature earns its place.
4. **bash-compatible subset** — vish doesn't need to run *every* bash script, but common constructs (`if`, `for`, `while`, functions, `$()`), well-known flags (`-e`, `-n`, `-c`), and the interactive experience should feel familiar.

## Roadmap

### Phase 1 — Standalone VIBIX Binary
- [ ] Extract shell code from `vibix_shell.inc` into a standalone flat binary
- [ ] Same feature set: readline, history, command dispatch, builtins
- [ ] VIBIT (init) can exec it as the interactive user shell
- [ ] Kernel syscall wrappers for VIBIX

### Phase 2 — Pipes & Redirection
- [ ] `cmd1 | cmd2` piping
- [ ] `> file`, `>> file`, `< file` redirection
- [ ] `2>`, `2>&1` stderr redirection
- [ ] Proper file descriptor plumbing

### Phase 3 — Linux ELF Target
- [ ] Rust core compiles for Linux
- [ ] Linux syscall backend (separate module)
- [ ] Same command parsing, expansion, job control as VIBIX
- [ ] `make test` runs on Linux with full CI

### Phase 4 — Environment Variable Expansion
- [ ] `$VAR`, `${VAR}`, `${VAR:-default}`
- [ ] `export`, `unset` builtins
- [ ] `$PATH` for command lookup
- [ ] `$HOME`, `$PWD`, `$?`, `$0`–`$9`, `$#`, `$@`
- [ ] Environment inheritance from parent process

### Phase 5 — Job Control
- [ ] Background execution (`cmd &`)
- [ ] `jobs` builtin
- [ ] `fg`, `bg` builtins
- [ ] SIGTSTP/Ctrl-Z handling
- [ ] Process groups and terminal ownership

### Phase 6 — Tab Completion
- [ ] Command name completion
- [ ] File path completion
- [ ] Variable name completion
- [ ] Configurable completion hooks

### Phase 7 — Scripting & bash Compatibility
- [ ] `vish script.vsh` / `#!/usr/bin/vish` shebang
- [ ] `if`/`then`/`else`/`fi`
- [ ] `for`/`while`/`until` loops
- [ ] Functions
- [ ] `case`/`esac`
- [ ] Arithmetic expansion `$((...))`
- [ ] Test command `[ ]`
- [ ] bash-compatible flag parsing subset

## Non-Goals (for now)

- POSIX shell compliance certification
- Interactive debugger / GUI
- Built-in file manager or editor
- Network transparency
- Plugin system
- Unicode beyond UTF-8 passthrough

## Success Criteria

- vish is the default interactive shell on VIBIX
- `make test` passes on Linux with >90% line coverage
- Common bash scripts (build scripts, simple tools) run unmodified under `vish -c`
- No regressions across Linux and VIBIX backends
