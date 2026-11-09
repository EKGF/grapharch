"""Publish only the already-verified native packages, then their exact-version CLI."""

import argparse
import json
import subprocess
from pathlib import Path

from distribute import PLATFORMS, npm_command, verify_release
from registry_state import npm_published

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
verify_release(args.output)
for item in PLATFORMS:
    manifest = json.loads(
        (args.output / f"grapharch-{item['target']}.json").read_text()
    )
    if npm_published(manifest['npm_package'], manifest['version'], args.output / manifest['npm_archive']):
        print(f"Already published identical npm artifact: {manifest['npm_package']}")
        continue
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
if not npm_published('grapharch', main['version'], args.output / main['archive']):
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
