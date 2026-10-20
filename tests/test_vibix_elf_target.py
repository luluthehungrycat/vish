import os
import struct
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_vibix_elf_is_loadable_by_vibix(tmp_path):
    target_dir = tmp_path / "cargo-target"
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)

    subprocess.run(["make", "elf"], cwd=ROOT, env=env, check=True)
    elf = target_dir / "x86_64-unknown-none" / "release" / "vibix"
    data = elf.read_bytes()

    assert data[:4] == b"\x7fELF"
    assert struct.unpack_from("<H", data, 16)[0] == 2  # ET_EXEC
    entry = struct.unpack_from("<Q", data, 24)[0]
    phoff = struct.unpack_from("<Q", data, 32)[0]
    phentsize, phnum = struct.unpack_from("<HH", data, 54)

    executable_segments = []
    for index in range(phnum):
        offset = phoff + index * phentsize
        p_type, flags, _, vaddr, _, _, memsz, _ = struct.unpack_from("<IIQQQQQQ", data, offset)
        if p_type == 1:  # PT_LOAD
            assert vaddr >= 0x2000000
            if flags & 1:  # PF_X
                executable_segments.append((vaddr, vaddr + memsz))

    assert any(start <= entry < end for start, end in executable_segments)

    for wrapper in ("vibix_syscall_read", "vibix_syscall_write", "vibix_syscall_brk"):
        disassembly = subprocess.run(
            ["objdump", "-d", f"--disassemble={wrapper}", str(elf)],
            check=True, capture_output=True, text=True,
        ).stdout
        for register in ("rbx", "rbp", "r12", "r13", "r14", "r15"):
            assert f"push   %{register}" in disassembly
            assert f"pop    %{register}" in disassembly

    subprocess.run(["make", "elf-probe"], cwd=ROOT, env=env, check=True)
    probe = target_dir / "x86_64-unknown-none" / "release" / "vibix_probe"
    probe_main = subprocess.run(
        ["objdump", "-d", "--disassemble=vibix_probe_main", str(probe)],
        check=True, capture_output=True, text=True,
    ).stdout
    assert "pause" in probe_main
    assert "syscall" not in probe_main
    for wrapper in ("vibix_probe_write",):
        disassembly = subprocess.run(
            ["objdump", "-d", f"--disassemble={wrapper}", str(probe)],
            check=True, capture_output=True, text=True,
        ).stdout
        for register in ("rbx", "rbp", "r12", "r13", "r14", "r15"):
            assert f"push   %{register}" in disassembly
            assert f"pop    %{register}" in disassembly
