"""Install release verification dependencies with bounded package-manager I/O."""

import subprocess
import sys


def install(*, apt=("sudo", "apt-get"), timeout=120):
    options = (
        "-o", "Acquire::Retries=0",
        "-o", "Acquire::http::Timeout=30",
        "-o", "Acquire::https::Timeout=30",
    )
    for operation in (
        ("update",),
        ("install", "-y", "clang", "libclang-dev", "pkg-config"),
    ):
        subprocess.run([*apt, *options, *operation], check=True, timeout=timeout)


if __name__ == "__main__":
    try:
        install()
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, OSError) as error:
        print(f"GraphArch native dependency installation failed: {error}", file=sys.stderr)
        sys.exit(1)
