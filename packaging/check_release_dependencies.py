"""Reject transitive native TLS before building portable release executables."""

import subprocess
import sys

FORBIDDEN = {"openssl", "openssl-sys", "native-tls", "hyper-tls"}
TARGETS = ["x86_64-unknown-linux-gnu", "aarch64-unknown-linux-gnu"]


def validate_tree(tree, target):
    names = {line.split()[0] for line in tree.splitlines() if line.strip()}
    forbidden = sorted(names & FORBIDDEN)
    if forbidden:
        raise ValueError(
            f"{target} enables native TLS dependencies: {', '.join(forbidden)}; portable binaries must use system-trusted Rust TLS"
        )


def main():
    for target in TARGETS:
        tree = subprocess.check_output(
            [
                "cargo",
                "tree",
                "--locked",
                "--color",
                "never",
                "--target",
                target,
                "--edges",
                "normal,build",
                "--prefix",
                "none",
                "--format",
                "{p}",
            ],
            text=True,
        )
        validate_tree(tree, target)
        print(f"Portable TLS dependency graph verified: {target}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"GraphArch release dependencies: {error}", file=sys.stderr)
        sys.exit(1)
