"""Run the pinned CLI with explicit arguments and consume its publication report."""

import json
import os
import signal
import subprocess
import sys
from pathlib import Path


def required(name):
    value = os.environ.get(name, "")
    if not value or "\n" in value or "\r" in value or "\x00" in value:
        raise ValueError(
            f"{name.removeprefix('GRAPHARCH_').lower()} must be a nonempty single-line value"
        )
    return value


def configuration():
    mode = required("GRAPHARCH_MODE")
    if mode == "install":
        return None
    if mode != "build-family":
        raise ValueError("mode must be build-family or install")
    kind = required("GRAPHARCH_SOURCE_KIND")
    if kind not in {"directory", "checkout"}:
        raise ValueError("source-kind must be directory or checkout")
    arguments = ["publication", "build-family"]
    option = "--source" if kind == "directory" else "--local-repository"
    arguments.extend([option, required("GRAPHARCH_SOURCE")])
    overrides = [
        os.environ.get(f"GRAPHARCH_{name}", "")
        for name in ["DEFINITIONS", "PROJECT", "TARGET"]
    ]
    if any(overrides):
        if not all(overrides):
            raise ValueError(
                "definitions, project and target must be supplied together, "
                "or omitted to discover repository hints"
            )
        for option, name in [
            ("--definitions", "DEFINITIONS"),
            ("--project", "PROJECT"),
            ("--target", "TARGET"),
        ]:
            arguments.extend([option, required(f"GRAPHARCH_{name}")])
    arguments.extend(["--output-root", required("GRAPHARCH_OUTPUT")])
    return arguments


def main():
    arguments = configuration()
    if arguments is None or sys.argv[1:] == ["validate"]:
        return 0
    command = [required("GRAPHARCH_BINARY"), *arguments]
    command.append("--json")
    result = subprocess.run(command, stdout=subprocess.PIPE, text=True, check=False)
    if result.returncode != 0:
        return result.returncode
    report = json.loads(result.stdout)
    if (
        not isinstance(report, dict)
        or type(report.get("schema_version")) is not int
        or report["schema_version"] != 1
    ):
        raise ValueError("Unsupported GraphArch publication report schema")
    outputs = {}
    output_root = Path(required("GRAPHARCH_OUTPUT")).resolve()
    for key in ["publication_path", "pdf_path", "kg_path"]:
        value = report.get(key)
        if not isinstance(value, str) or not value:
            raise ValueError(f"Publication report requires {key}")
        if any(character in value for character in "\n\r\x00"):
            raise ValueError(f"Invalid path in publication report: {key}")
        path = Path(value).resolve()
        if not path.is_relative_to(output_root) or path == output_root:
            raise ValueError(
                f"Publication report path must be under output root: {key}"
            )
        if key == "publication_path":
            index = (path / "index.html").resolve()
            if (
                not path.is_dir()
                or not index.is_file()
                or not index.is_relative_to(path)
            ):
                raise ValueError("Publication directory must contain index.html")
        elif not path.is_file():
            raise ValueError(f"Publication report file does not exist: {key}")
        outputs[key.replace("_", "-")] = str(path)
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        for key, value in outputs.items():
            output.write(f"{key}={value}\n")
    print(f"GraphArch publication: {report['publication_path']}")
    return 0


if __name__ == "__main__":
    try:
        status = main()
    except (OSError, ValueError) as error:
        print(f"GraphArch action: {error}", file=sys.stderr)
        sys.exit(1)
    if status < 0:
        # Preserve signal termination instead of converting it into success or a generic error.
        signal.signal(-status, signal.SIG_DFL)
        os.kill(os.getpid(), -status)
    sys.exit(status)
