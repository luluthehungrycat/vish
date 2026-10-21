import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TRACKED_FLAT_SOURCE = ROOT / "src" / "vibix" / "vish"


def test_nasm_and_default_build_to_requested_paths_without_clobbering_source(tmp_path):
    original = TRACKED_FLAT_SOURCE.read_bytes()
    default_output = tmp_path / "default.bin"
    nasm_output = tmp_path / "nasm.bin"

    subprocess.run(
        ["make", f"VISH_BIN={default_output}"], cwd=ROOT, check=True,
        capture_output=True, text=True,
    )
    subprocess.run(
        ["make", "nasm", f"VISH_BIN={nasm_output}"], cwd=ROOT, check=True,
        capture_output=True, text=True,
    )

    assert default_output.is_file() and default_output.stat().st_size > 0
    assert nasm_output.is_file() and nasm_output.stat().st_size > 0
    assert default_output.read_bytes() == nasm_output.read_bytes()
    assert TRACKED_FLAT_SOURCE.read_bytes() == original
