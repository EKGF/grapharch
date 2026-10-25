"""Install the built wheel/npm packages offline and exercise their CLI entry points."""

import argparse
import json
import os
import platform
import subprocess
import tempfile
import venv
import zipfile
from pathlib import Path

from distribute import (
    binary_name,
    npm_command,
    verify,
    version,
)
from distribute import (
    platform as release_platform,
)


def checked(command: list[str], cwd: Path | None = None) -> str:
    return subprocess.check_output(
        command, cwd=cwd, text=True, stderr=subprocess.STDOUT
    )


def smoke(output: Path, target: str) -> None:
    output = output.resolve()
    item = release_platform(target)
    host_os = {"Darwin": "darwin", "Linux": "linux", "Windows": "win32"}.get(
        platform.system()
    )
    host_cpu = {
        "aarch64": "arm64",
        "arm64": "arm64",
        "x86_64": "x64",
        "amd64": "x64",
    }.get(platform.machine().lower())
    if (host_os, host_cpu) != (item["os"], item["cpu"]):
        raise ValueError(
            "install smoke tests must execute on the artifact's native platform"
        )
    verify(output, target)
    manifest = json.loads((output / f"grapharch-{target}.json").read_text())
    expected = f"grapharch {version()}"
    with tempfile.TemporaryDirectory(prefix="grapharch-package-smoke-") as temporary:
        root = Path(temporary)
        with zipfile.ZipFile(output / manifest["archive"]) as archive:
            binary = root / binary_name(item)
            binary.write_bytes(archive.read(binary_name(item)))
            binary.chmod(0o755)
        if checked([str(binary), "--version"]).strip() != expected:
            raise ValueError("standalone archive reports a different version")
        environment = root / "python environment"
        venv.EnvBuilder(with_pip=True).create(environment)
        scripts = environment / ("Scripts" if os.name == "nt" else "bin")
        python = scripts / ("python.exe" if os.name == "nt" else "python")
        checked(
            [
                str(python),
                "-m",
                "pip",
                "install",
                "--no-index",
                "--no-deps",
                str(output / manifest["wheel"]),
            ]
        )
        command = scripts / binary_name(item)
        if checked(
            [str(command), "--version"]
        ).strip() != expected or "build-family" not in checked(
            [str(command), "publication", "--help"]
        ):
            raise ValueError("installed Python wheel does not expose the expected CLI")
        project = root / "npm project"
        project.mkdir()
        (project / "package.json").write_text(
            json.dumps(
                {
                    "name": "grapharch-install-smoke",
                    "private": True,
                    "scripts": {"grapharch": "grapharch"},
                }
            )
        )
        checked(
            npm_command()
            + [
                "install",
                "--offline",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
                "--package-lock=false",
                str(output / manifest["npm_archive"]),
                str(output / f"grapharch-{version()}.tgz"),
            ],
            project,
        )
        response = checked(
            npm_command() + ["run", "--silent", "grapharch", "--", "--version"], project
        )
        if response.strip() != expected:
            raise ValueError("installed npm package does not expose the expected CLI")
        if "build-family" not in checked(
            npm_command()
            + ["run", "--silent", "grapharch", "--", "publication", "--help"],
            project,
        ):
            raise ValueError("installed npm package loses CLI arguments")
    print(f"Offline standalone, Python, and npm install smoke tests passed: {target}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    smoke(args.output, args.target)
