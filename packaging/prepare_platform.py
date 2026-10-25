"""Assemble a single native build result without shell-specific glob behavior."""

import argparse
import shutil
from pathlib import Path

from distribute import build, pack_npm, version

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--target", required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--clean-main", action="store_true")
args = parser.parse_args()
if args.clean_main:
    shutil.rmtree(args.output / "npm/grapharch")
    (args.output / f"grapharch-{version()}.tgz").unlink()
    (args.output / "grapharch-npm.json").unlink()
else:
    wheels = list((args.output / "wheels").glob("*.whl"))
    if len(wheels) != 1:
        raise ValueError("one native wheel must be built per matrix job")
    build(None, wheels[0], args.target, args.output)
    pack_npm(args.output, args.target)
