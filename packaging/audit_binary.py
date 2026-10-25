"""Reject release executables that depend on unshipped libraries or newer libc."""

import argparse
import re
import subprocess
import sys
from pathlib import Path


def audit(binary: Path, target: str) -> None:
    if "linux" in target:
        dynamic = subprocess.check_output(
            ["readelf", "--dynamic", str(binary)], text=True
        )
        needed = re.findall(r"Shared library: \[(.*?)\]", dynamic)
        allowed = {
            "libc.so.6",
            "libm.so.6",
            "libpthread.so.0",
            "libdl.so.2",
            "librt.so.1",
            "libgcc_s.so.1",
            "ld-linux-x86-64.so.2",
            "ld-linux-aarch64.so.1",
        }
        if not needed or set(needed) - allowed:
            raise ValueError(
                f"unshipped Linux runtime dependencies: {sorted(set(needed) - allowed)}"
            )
        versions = subprocess.check_output(
            ["readelf", "--version-info", str(binary)], text=True
        )
        if "GLIBCXX_" in versions or "CXXABI_" in versions:
            raise ValueError("C++ runtime must be statically linked")
        glibc = [
            tuple(map(int, v.split(".")))
            for v in re.findall(r"GLIBC_(\d+\.\d+(?:\.\d+)?)", versions)
        ]
        if not glibc or max(glibc) > (2, 28):
            raise ValueError(
                f"Linux executable exceeds glibc 2.28 baseline: {max(glibc, default=None)}"
            )
    elif "darwin" in target:
        dependencies = subprocess.check_output(
            ["otool", "-L", str(binary)], text=True
        ).splitlines()[1:]
        unexpected = [
            line.strip()
            for line in dependencies
            if not line.strip().startswith(("/usr/lib/", "/System/Library/"))
        ]
        if unexpected:
            raise ValueError(f"unshipped macOS runtime dependencies: {unexpected}")
    elif "windows" in target:
        output = subprocess.check_output(
            ["dumpbin", "/dependents", str(binary)], text=True
        )
        dependencies = re.findall(
            r"^\s+([^\s]+\.dll)\s*$", output, re.MULTILINE | re.IGNORECASE
        )
        if not dependencies:
            raise ValueError("could not inspect Windows executable dependencies")
        if any(
            re.search(r"(?:vcruntime|msvcp|libssl|libcrypto)", name, re.IGNORECASE)
            for name in dependencies
        ):
            raise ValueError(
                f"Windows executable needs an unshipped runtime: {dependencies}"
            )
    else:
        raise ValueError(f"unsupported release target: {target}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--target", required=True)
    arguments = parser.parse_args()
    try:
        audit(arguments.binary, arguments.target)
    except (ValueError, subprocess.CalledProcessError, OSError) as error:
        print(f"GraphArch binary audit: {error}", file=sys.stderr)
        sys.exit(1)
