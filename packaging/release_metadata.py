"""Validate the selected release ref before building or publishing anything."""

import json
import os
import subprocess
from pathlib import Path

from distribute import PLATFORMS, check, version

check()
release_version = version()
if (
    os.environ.get("GITHUB_REF_TYPE") == "tag"
    and os.environ["GITHUB_REF_NAME"] != f"v{release_version}"
):
    raise ValueError("release tag does not match Cargo.toml")
head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
tag = f"refs/tags/v{release_version}"
probe = subprocess.run(["git", "show-ref", "--verify", "--quiet", tag], check=False)
if probe.returncode == 0:
    tagged = subprocess.check_output(
        ["git", "rev-parse", f"{tag}^{{commit}}"], text=True
    ).strip()
    if tagged != head:
        raise ValueError("version tag already refers to a different commit")
elif probe.returncode != 1:
    raise ValueError("cannot inspect existing release tag")
publish = (
    os.environ.get("REQUEST_PUBLISH") == "true"
    or os.environ.get("GITHUB_EVENT_NAME") == "push"
)
with Path(os.environ["GITHUB_OUTPUT"]).open("a") as output:
    output.write(
        f"version={release_version}\nmatrix={json.dumps({'include': PLATFORMS})}\npublish={str(publish).lower()}\n"
    )
