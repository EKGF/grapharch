"""Checksum-verified installation of exact release binaries for GitHub Actions."""

import hashlib
import os
import platform
import re
import subprocess
import sys
import urllib.request
import zipfile
from pathlib import Path


def target():
    systems = {
        "Linux": "unknown-linux-gnu",
        "Darwin": "apple-darwin",
        "Windows": "pc-windows-msvc",
    }
    cpus = {
        "x86_64": "x86_64",
        "amd64": "x86_64",
        "arm64": "aarch64",
        "aarch64": "aarch64",
    }
    system, cpu = platform.system(), platform.machine().lower()
    if system not in systems or cpu not in cpus:
        raise ValueError(f"Unsupported platform: {system}/{cpu}")
    if system == "Linux":
        libc, version = platform.libc_ver()
        if libc != "glibc" or tuple(map(int, version.split("."))) < (2, 28):
            raise ValueError("GraphArch requires Linux with glibc 2.28 or newer")
    return f"{cpus[cpu]}-{systems[system]}"


def main():
    version = os.environ["GRAPHARCH_VERSION"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("version must be an exact MAJOR.MINOR.PATCH release without v")
    triple = target()
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write(f"target={triple}\n")
    if sys.argv[1] == "identify":
        return
    if sys.argv[1] != "install":
        raise ValueError("Expected identify or install")
    directory = Path(os.environ["RUNNER_TOOL_CACHE"]) / "grapharch" / version / triple
    directory.mkdir(parents=True, exist_ok=True)
    archive_name = f"grapharch-{triple}.zip"
    base = (
        f"https://github.com/EKGF/grapharch/releases/download/v{version}/{archive_name}"
    )
    # Fetch the release checksum even on a cache hit; a cache never replaces verification.
    with urllib.request.urlopen(base + ".sha256", timeout=60) as response:
        checksum = response.read(4096).decode().strip()
    match = re.fullmatch(r"([a-f0-9]{64})\s+\*?" + re.escape(archive_name), checksum)
    if not match:
        raise ValueError("Invalid release checksum file")
    archive = directory / archive_name
    if not archive.exists():
        with (
            urllib.request.urlopen(base, timeout=60) as response,
            archive.open("wb") as output,
        ):
            while chunk := response.read(1024 * 1024):
                output.write(chunk)
    with archive.open("rb") as source:
        actual_checksum = hashlib.file_digest(source, "sha256").hexdigest()
    if actual_checksum != match[1]:
        raise ValueError(
            "Release archive checksum mismatch; remove the corrupt cache entry"
        )
    name = "grapharch.exe" if platform.system() == "Windows" else "grapharch"
    binary = directory / name
    with zipfile.ZipFile(archive) as source:
        entries = [entry for entry in source.infolist() if entry.filename == name]
        if len(entries) != 1 or entries[0].file_size > 512 * 1024 * 1024:
            raise ValueError("Invalid standalone release archive")
        binary.write_bytes(source.read(entries[0]))
    binary.chmod(0o755)
    result = subprocess.run(
        [str(binary), "--version"], check=True, text=True, capture_output=True
    )
    if result.stdout.strip() != f"grapharch {version}":
        raise ValueError("Installed binary version does not match requested release")
    with open(os.environ["GITHUB_PATH"], "a") as output:
        output.write(str(directory) + "\n")
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write(f"version={version}\nbinary-path={binary}\n")


if __name__ == "__main__":
    try:
        main()
    except (
        OSError,
        ValueError,
        subprocess.CalledProcessError,
        zipfile.BadZipFile,
    ) as error:
        print(f"GraphArch setup: {error}", file=sys.stderr)
        sys.exit(1)
