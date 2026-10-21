#!/usr/bin/env python3
"""Run VISH external commands until leaked zombies would exhaust the process table.

Run with ``python3 tests/qemu_external_wait.py`` in a workspace containing
neighboring ``vibix`` and ``vibit`` checkouts. The fixture restores every
generated VIBIX/VIBIT/VISH artifact it touches.
"""

import argparse
import os
import re
import selectors
import socket
import subprocess
import sys
import time
from pathlib import Path

VISH_ROOT = Path(__file__).resolve().parents[1]
VIBIX_ROOT = Path(os.environ.get("VIBIX_ROOT", VISH_ROOT.parent / "vibix")).resolve()
VIBIT_ROOT = Path(os.environ.get("VIBIT_ROOT", VIBIX_ROOT.parent / "vibit")).resolve()
QEMU = os.environ.get("QEMU", "/workspace/.onboarding/bin/qemu-system-x86_64")


def _snapshot_paths():
    generated = (
        "kernel64_entry.o", "interrupts.o", "syscall_entry.o", "context_switch.o",
        "kernel64.elf", "kernel64.bin", "boot.o", "vibix.elf",
    )
    return [
        VIBIX_ROOT / "userspace/initramfs.tar",
        VIBIX_ROOT / "userspace/vibix_blob.bin",
        VIBIX_ROOT / "kernel_rust/target/x86_64-unknown-none/release/libvibix_kernel.a",
        VIBIT_ROOT / "vibit.bin",
        VISH_ROOT / "vish.bin",
        *(VIBIX_ROOT / name for name in generated),
    ]


def _wait_for(selector, output, predicate, timeout):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        text = output.decode("ascii", errors="replace")
        if predicate(text):
            return text
        events = selector.select(min(0.05, deadline - time.monotonic()))
        for key, _ in events:
            chunk = key.fileobj.recv(8192)
            if not chunk:
                raise RuntimeError("QEMU serial connection closed")
            output.extend(chunk)
    raise TimeoutError(output.decode("ascii", errors="replace")[-2000:])


def _run(rounds):
    if not VIBIX_ROOT.is_dir() or not VIBIT_ROOT.is_dir():
        raise SystemExit("expected sibling vibix and vibit checkouts")
    sys.path.insert(0, str(VIBIX_ROOT))
    from test_rust_vish import ArtifactSnapshot

    with ArtifactSnapshot(_snapshot_paths()):
        subprocess.run(
            ["make", "-B", "DEBUG=1", "INIT=vibit", "vibix.elf"],
            cwd=VIBIX_ROOT, check=True,
        )

        with socket.socket() as probe:
            probe.bind(("127.0.0.1", 0))
            port = probe.getsockname()[1]
        proc = subprocess.Popen(
            [QEMU, "-accel", "tcg", "-kernel", "vibix.elf", "-serial",
             f"tcp:127.0.0.1:{port},server=on,wait=on", "-display", "none", "-m",
             "512M", "-no-reboot", "-no-shutdown"],
            cwd=VIBIX_ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        try:
            conn = None
            deadline = time.monotonic() + 20
            while conn is None and time.monotonic() < deadline:
                try:
                    conn = socket.create_connection(("127.0.0.1", port), timeout=0.2)
                except OSError:
                    if proc.poll() is not None:
                        raise RuntimeError("QEMU exited before opening serial")
            if conn is None:
                raise TimeoutError("could not connect to QEMU serial")

            conn.setblocking(False)
            selector = selectors.DefaultSelector()
            selector.register(conn, selectors.EVENT_READ)
            output = bytearray()
            prompts = re.compile(r"vish\$ ")
            _wait_for(selector, output, lambda text: len(prompts.findall(text)) >= 1, 30)

            for iteration in range(rounds):
                before = output.decode("ascii", errors="replace")
                prompt_count = len(prompts.findall(before))
                goodbye_count = before.count("goodbye")
                conn.sendall(b"/bin/vish\n")
                _wait_for(
                    selector, output,
                    lambda text: len(prompts.findall(text)) >= prompt_count + 1,
                    5,
                )
                conn.sendall(b"exit\n")
                text = _wait_for(
                    selector, output,
                    lambda value: (len(prompts.findall(value)) >= prompt_count + 2
                                   and value.count("goodbye") >= goodbye_count + 1),
                    5,
                )
                if "vish: fork failed" in text:
                    raise RuntimeError(f"fork failed before round {iteration + 1}")
                if "vish: waitpid failed" in text:
                    raise RuntimeError(f"waitpid failed during round {iteration + 1}")
            selector.close()
            conn.close()
            print(f"PASS: {rounds} external child shells exited without fork exhaustion")
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait(timeout=5)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rounds", type=int, default=65)
    args = parser.parse_args()
    if args.rounds < 2:
        parser.error("--rounds must be at least 2")
    _run(args.rounds)
