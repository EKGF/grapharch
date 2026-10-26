"""Select the explicit native runtime link policy for a release target."""

import argparse
import os
from pathlib import Path

from distribute import platform


def environment(target):
    item = platform(target)
    if item["os"] == "linux":
        # Oxigraph builds its API and RocksDB with independent cc::Builds.
        # Unbundled static declarations carry a -bundle modifier, which rustc
        # rejects when repeated. Bundled static declarations deduplicate and
        # still produce a standalone executable without a shared C++ runtime.
        return {"CXXSTDLIB": "static=stdc++"}
    return {}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    with Path(os.environ["GITHUB_ENV"]).open("a") as output:
        for name, value in environment(args.target).items():
            output.write(f"{name}={value}\n")
