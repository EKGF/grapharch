"""Publish only the already-verified native packages, then their exact-version CLI."""

import argparse
import json
import subprocess
from pathlib import Path

from distribute import PLATFORMS, npm_command, verify_release

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
verify_release(args.output)
for item in PLATFORMS:
    manifest = json.loads(
        (args.output / f"grapharch-{item['target']}.json").read_text()
    )
    subprocess.run(
        npm_command()
        + [
            "publish",
            "--access",
            "public",
            "--provenance",
            str(args.output / manifest["npm_archive"]),
        ],
        check=True,
    )
main = json.loads((args.output / "grapharch-npm.json").read_text())
subprocess.run(
    npm_command()
    + [
        "publish",
        "--access",
        "public",
        "--provenance",
        str(args.output / main["archive"]),
    ],
    check=True,
)
