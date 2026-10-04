# vish — Build System
#
# Phase 1: Standalone NASM flat binary for VIBIX
#
# Targets:
#   make          — build vish.bin (flat binary for VIBIX)
#   make clean    — remove build artifacts
#   make size     — show binary size info
#   make hexdump  — show first bytes of binary
#   make strings  — show all printable strings in binary
#

NASM      := nasm
NASMFLAGS := -f bin
CARGO     ?= cargo
RUST_TOOLCHAIN ?= stable
RUST_TARGET ?= x86_64-unknown-none
VIBIX_RUSTFLAGS = -C relocation-model=static -C link-arg=--image-base=0x2000000

SRC       := src/vibix/vish.asm
VISH_BIN  ?= vish.bin
BIN       := $(VISH_BIN)

.PHONY: all nasm elf elf-probe clean size hexdump strings

all: $(BIN)
nasm: $(BIN)

elf:
	RUSTFLAGS="$(VIBIX_RUSTFLAGS)" $(CARGO) +$(RUST_TOOLCHAIN) build --locked --no-default-features --bin vibix --target $(RUST_TARGET) --release

elf-probe:
	RUSTFLAGS="$(VIBIX_RUSTFLAGS)" $(CARGO) +$(RUST_TOOLCHAIN) build --locked --no-default-features --bin vibix_probe --target $(RUST_TARGET) --release

$(BIN): $(SRC)
	$(NASM) $(NASMFLAGS) $< -o $@
	@echo "Built: $@ ($$(wc -c < $@) bytes)"
	@echo "Entry: $$($(NASM) -f bin -l /dev/stdout -o /dev/null $< 2>/dev/null | head -3)"

clean:
	rm -f $(BIN)

size: $(BIN)
	@ls -la $(BIN)
	@echo ""
	@echo "Binary size: $$(wc -c < $(BIN)) bytes"
	@echo "Page count:  $$(( ($$(wc -c < $(BIN)) + 4095) / 4096 )) pages"

hexdump: $(BIN)
	@xxd $(BIN) | head -30

strings: $(BIN)
	@strings -n 4 $(BIN) | sort -u
