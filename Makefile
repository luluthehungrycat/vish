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

SRC       := src/vibix/vish.asm
BIN       := vish.bin

.PHONY: all clean size hexdump strings

all: $(BIN)

$(BIN): $(SRC)
	$(NASM) $(NASMFLAGS) $< -o $@
	@echo "Built: $@ ($$(wc -c < $@) bytes)"
	@echo "Entry: $$($(NASM) -f bin -l /dev/stdout $< 2>/dev/null | head -3)"

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
