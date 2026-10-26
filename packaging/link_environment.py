"""Select the explicit native runtime link policy for a release target."""

import argparse
import os
from pathlib import Path
import shlex
import subprocess

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


def linux_runtime_environment(*, compiler=None, previous=None):
    previous = os.environ if previous is None else previous
    compiler = shlex.split(previous.get("CXX", "c++")) if compiler is None else compiler
    archive = Path(subprocess.check_output(
        [*compiler, "-print-file-name=libstdc++.a"], text=True, timeout=30,
    ).strip())
    if not archive.is_absolute() or not archive.is_file():
        raise ValueError(f"C++ compiler did not identify a static runtime archive: {archive}")
    flags = (previous["CARGO_ENCODED_RUSTFLAGS"].split("\x1f")
             if previous.get("CARGO_ENCODED_RUSTFLAGS")
             else previous.get("RUSTFLAGS", "").split())
    flags.extend(["-L", f"native={archive.parent}"])
    return {"CXXSTDLIB": "static=stdc++", "CARGO_ENCODED_RUSTFLAGS": "\x1f".join(flags)}


def write_shell(path, values):
    path.write_text("".join(f"export {name}={shlex.quote(value)}\n"
                            for name, value in values.items()))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True)
    parser.add_argument("--shell-output", type=Path)
    args = parser.parse_args()
    values = environment(args.target)
    if args.shell_output:
        if platform(args.target)["os"] != "linux":
            raise ValueError("Compiler runtime search paths are only configured inside Linux builds")
        write_shell(args.shell_output, linux_runtime_environment())
    else:
        with Path(os.environ["GITHUB_ENV"]).open("a") as output:
            for name, value in values.items():
                output.write(f"{name}={value}\n")
