"""Install release verification dependencies with bounded package-manager I/O."""

import subprocess
import sys


def install(*, apt=("sudo", "apt-get"), timeout=600):
    options = (
        "-o", "Acquire::Retries=5",
        "-o", "Acquire::http::Timeout=60",
        "-o", "Acquire::https::Timeout=60",
    )
    for operation in (
        ("update",),
        ("install", "-y", "--no-install-recommends", "clang", "libclang-dev", "pkg-config", "libssl-dev"),
    ):
        subprocess.run([*apt, *options, *operation], check=True, timeout=timeout)


if __name__ == "__main__":
    try:
        install()
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, OSError) as error:
        print(f"GraphArch native dependency installation failed: {error}", file=sys.stderr)
        sys.exit(1)
