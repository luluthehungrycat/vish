import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "src" / "vibix" / "vish.asm"
FULL_CLOBBER_REGISTERS = [
    "rbx", "rbp", "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10",
    "r11", "r12", "r13", "r14", "r15",
]


def _registers(body, instruction):
    return re.findall(rf"^\s*{instruction}\s+(r(?:[0-9]+|bx|bp|cx|dx|si|di))\s*$", body, re.M)


def test_blocking_syscall_wrappers_preserve_full_clobber_state():
    source = SOURCE.read_text()
    save = re.search(r"%macro save_syscall_registers 0(.*?)%endmacro", source, re.S)
    restore = re.search(r"%macro restore_syscall_registers 0(.*?)%endmacro", source, re.S)
    assert save and restore
    assert _registers(save.group(1), "push") == FULL_CLOBBER_REGISTERS
    assert _registers(restore.group(1), "pop") == FULL_CLOBBER_REGISTERS[::-1]

    for name in ("sys_write", "sys_read"):
        wrapper = re.search(rf"^{name}:(.*?)(?=^\w[\w.]*:|\Z)", source, re.M | re.S)
        assert wrapper, f"missing {name} wrapper"
        body = wrapper.group(1)
        assert "save_syscall_registers" in body
        assert "syscall" in body
        assert "restore_syscall_registers" in body

    read = re.search(r"^sys_read:(.*?)(?=^\w[\w.]*:|\Z)", source, re.M | re.S).group(1)
    assert "mov rdx, [rsp + 72]" in read
    assert "mov rsi, [rsp + 64]" in read


def test_external_command_retries_wait_until_the_child_is_reaped():
    source = SOURCE.read_text()
    helper = re.search(r"^try_find_and_exec:(.*?)(?=^\w[\w.]*:|\Z)", source, re.M | re.S)
    assert helper
    body = helper.group(1)
    assert "test rax, rax\n    js .fork_failed" in body
    assert "mov [rel saved_child_pid], rax" in body
    assert re.search(r"\.wait_child:\n(?:(?!^\.child:).)*?jz \.wait_child", body, re.M | re.S)
    assert "js .wait_failed" in body
    assert "cmp rax, [rel saved_child_pid]" in body
    assert "str_fork_failed" in body
    assert "str_wait_failed" in body
