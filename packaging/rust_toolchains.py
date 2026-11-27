"""Leave only the repository's pinned Rust toolchain installed on a CI runner.

Swatinem/rust-cache hashes the version of every installed rustup toolchain into
its cache key. Hosted runner images preinstall a floating ``stable`` toolchain
that the build never uses; each image update changed the key, so release builds
missed the cache although the pinned compiler and lockfile were identical.
"""

import argparse
import shutil
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def pinned_channel(root=ROOT):
    channel = tomllib.loads((root / "rust-toolchain.toml").read_text(encoding="utf-8"))[
        "toolchain"]["channel"]
    if not channel.startswith("nightly-") and not channel[:1].isdigit():
        raise ValueError(f"rust-toolchain.toml must pin an exact toolchain, not {channel!r}")
    return channel


def unpinned(listing, channel):
    """Installed toolchain names from ``rustup toolchain list`` other than the pin."""
    if listing.strip() == "no installed toolchains":
        return []
    names = [line.split()[0] for line in listing.splitlines() if line.strip()]
    return [name for name in names if name != channel and not name.startswith(channel + "-")]


def main():
    argparse.ArgumentParser(description=__doc__).parse_args()
    channel = pinned_channel()
    if shutil.which("rustup") is None:
        # A runner without rustup has no preinstalled toolchain to remove;
        # setup-rust-toolchain installs rustup and then only the pinned channel.
        print(f"rustup is not installed; only {channel} will be installed")
        return
    listing = subprocess.check_output(["rustup", "toolchain", "list"], text=True, cwd=ROOT.parent)
    for name in unpinned(listing, channel):
        print(f"Removing runner toolchain {name}; GraphArch builds with {channel}")
        subprocess.run(["rustup", "toolchain", "uninstall", name], check=True)


if __name__ == "__main__":
    main()
