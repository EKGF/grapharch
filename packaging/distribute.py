"""Build release channels from one verified platform executable (Python 3.11+)."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

import tomllib

ROOT = Path(__file__).resolve().parents[1]
PLATFORMS = json.loads((ROOT / "packaging/platforms.json").read_text())
REPOSITORY = "https://github.com/EKGF/grapharch"
MAX_BINARY = 512 * 1024 * 1024


def version() -> str:
    value = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    if not re.fullmatch(r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)", value):
        raise ValueError(
            "distribution currently requires a stable MAJOR.MINOR.PATCH version"
        )
    return value


def platform(target: str) -> dict:
    matches = [item for item in PLATFORMS if item["target"] == target]
    if len(matches) != 1:
        raise ValueError(f"unsupported target: {target}")
    return matches[0]


def sha256(path: Path) -> str:
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def check() -> None:
    expected = version()
    manifest = json.loads((ROOT / "npm/package.json").read_text())
    if manifest["version"] != expected:
        raise ValueError(
            "npm version differs from Cargo.toml; run packaging/distribute.py sync-version"
        )
    packages = {f"@grapharch/cli-{p['key']}": expected for p in PLATFORMS}
    if manifest["optionalDependencies"] != packages:
        raise ValueError(
            "npm platform dependencies differ from the declared platform matrix/version"
        )
    python = tomllib.loads((ROOT / "pyproject.toml").read_text())
    if python["project"]["name"] != "grapharch" or python["project"]["dynamic"] != [
        "version"
    ]:
        raise ValueError("PyPI must derive the grapharch version from Cargo.toml")
    if python["tool"]["maturin"]["bindings"] != "bin":
        raise ValueError("PyPI must distribute the CLI, not a separate implementation")


def sync_version() -> None:
    path = ROOT / "npm/package.json"
    manifest = json.loads(path.read_text())
    manifest["version"] = version()
    manifest["optionalDependencies"] = {
        f"@grapharch/cli-{p['key']}": version() for p in PLATFORMS
    }
    path.write_text(json.dumps(manifest, indent=2) + "\n")
    check()


def binary_name(item: dict) -> str:
    return "grapharch.exe" if item["os"] == "win32" else "grapharch"


def verify_machine(path: Path, item: dict) -> None:
    with path.open("rb") as stream:
        header = stream.read(4096)
    arm = item["cpu"] == "arm64"
    if item["os"] == "linux":
        valid = (
            len(header) >= 20
            and header[:6] == b"\x7fELF\x02\x01"
            and struct.unpack_from("<H", header, 18)[0] == (183 if arm else 62)
        )
    elif item["os"] == "darwin":
        valid = (
            len(header) >= 8
            and header[:4] == b"\xcf\xfa\xed\xfe"
            and struct.unpack_from("<I", header, 4)[0]
            == (0x0100000C if arm else 0x01000007)
        )
    else:
        offset = (
            struct.unpack_from("<I", header, 60)[0]
            if len(header) >= 64
            else len(header)
        )
        valid = (
            len(header) >= offset + 6
            and header[:2] == b"MZ"
            and header[offset : offset + 4] == b"PE\0\0"
            and struct.unpack_from("<H", header, offset + 4)[0]
            == (0xAA64 if arm else 0x8664)
        )
    if not valid:
        raise ValueError(f"binary machine format does not match {item['target']}")


def extract_binary(wheel: Path, item: dict, output: Path) -> Path:
    name = binary_name(item)
    with zipfile.ZipFile(wheel) as archive:
        candidates = [
            entry
            for entry in archive.infolist()
            if entry.filename.endswith(f".data/scripts/{name}")
        ]
        if len(candidates) != 1 or candidates[0].file_size > MAX_BINARY:
            raise ValueError(
                "wheel must contain exactly one bounded GraphArch CLI script"
            )
        if any(
            entry.filename.endswith((".so", ".dylib", ".dll"))
            for entry in archive.infolist()
        ):
            raise ValueError(
                "wheel contains bundled shared libraries; standalone/npm artifact would not be self-contained"
            )
        metadata = [
            entry
            for entry in archive.namelist()
            if entry.endswith(".dist-info/METADATA")
        ]
        if len(metadata) != 1:
            raise ValueError("wheel must contain one package metadata record")
        text = archive.read(metadata[0]).decode()
        if "Name: grapharch\n" not in text or f"Version: {version()}\n" not in text:
            raise ValueError("wheel name/version does not match this release")
        with archive.open(candidates[0]) as source, output.open("wb") as destination:
            shutil.copyfileobj(source, destination, 1024 * 1024)
    output.chmod(0o755)
    return output


def notices() -> list[Path]:
    files = sorted((ROOT / "packaging/licenses").glob("*.txt"))
    fonts = ROOT / "src/publication/render/fonts"
    font_notices = sorted(fonts.glob("LICENSE*"))
    if list(fonts.glob("*.ttf")) and not font_notices:
        raise ValueError("embedded font assets have no license notice")
    files.extend(font_notices)
    if len({p.name for p in files}) != len(files):
        raise ValueError("embedded asset license filenames must be unique")
    return files


def build(binary: Path | None, wheel: Path | None, target: str, output: Path) -> None:
    check()
    item = platform(target)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="grapharch-artifact-") as temporary:
        if wheel is not None:
            executable = extract_binary(
                wheel, item, Path(temporary) / binary_name(item)
            )
        else:
            if (
                binary is None
                or not binary.is_file()
                or binary.stat().st_size > MAX_BINARY
            ):
                raise ValueError("a bounded executable file is required")
            executable = binary
        verify_machine(executable, item)
        package = output / "npm" / f"cli-{item['key']}"
        package.mkdir(parents=True, exist_ok=False)
        (package / "bin").mkdir()
        shipped = package / "bin" / binary_name(item)
        shutil.copyfile(executable, shipped)
        shipped.chmod(0o755)
        metadata = {
            "name": f"@grapharch/cli-{item['key']}",
            "version": version(),
            "description": f"GraphArch executable for {item['key']}",
            "license": "MIT",
            "os": [item["os"]],
            "cpu": [item["cpu"]],
            "repository": {"type": "git", "url": f"git+{REPOSITORY}.git"},
            "files": ["bin/", "LICENSE", "licenses/"],
            "publishConfig": {"access": "public"},
        }
        if "libc" in item:
            metadata["libc"] = [item["libc"]]
        (package / "package.json").write_text(json.dumps(metadata, indent=2) + "\n")
        shutil.copyfile(ROOT / "LICENSE", package / "LICENSE")
        (package / "licenses").mkdir()
        for notice in notices():
            shutil.copyfile(notice, package / "licenses" / notice.name)
        archive = output / f"grapharch-{target}.zip"
        with zipfile.ZipFile(
            archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
        ) as zipped:
            info = zipfile.ZipInfo(binary_name(item), (1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = (stat.S_IFREG | 0o755) << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            with shipped.open("rb") as source, zipped.open(info, "w") as destination:
                shutil.copyfileobj(source, destination, 1024 * 1024)
            license_info = zipfile.ZipInfo("LICENSE", (1980, 1, 1, 0, 0, 0))
            license_info.create_system = 3
            license_info.external_attr = (stat.S_IFREG | 0o644) << 16
            license_info.compress_type = zipfile.ZIP_DEFLATED
            zipped.writestr(license_info, (ROOT / "LICENSE").read_bytes())
            for notice in notices():
                info = zipfile.ZipInfo(f"licenses/{notice.name}", (1980, 1, 1, 0, 0, 0))
                info.create_system = 3
                info.external_attr = (stat.S_IFREG | 0o644) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                zipped.writestr(info, notice.read_bytes())
        digest = sha256(archive)
        (output / f"{archive.name}.sha256").write_text(f"{digest}  {archive.name}\n")
        manifest = {
            "version": version(),
            "target": target,
            "platform": item["key"],
            "archive": archive.name,
            "archive_sha256": digest,
            "binary_sha256": sha256(shipped),
            "binary": binary_name(item),
            "npm_package": metadata["name"],
            "notices": {p.name: sha256(p) for p in notices()},
        }
        if wheel:
            destination = output / wheel.name
            if wheel.resolve() != destination.resolve():
                shutil.copyfile(wheel, destination)
            manifest["wheel"] = wheel.name
            manifest["wheel_sha256"] = sha256(destination)
        (output / f"grapharch-{target}.json").write_text(
            json.dumps(manifest, indent=2) + "\n"
        )
    verify(output, target)


def verify(output: Path, target: str) -> None:
    item = platform(target)
    manifest = json.loads((output / f"grapharch-{target}.json").read_text())
    if manifest["version"] != version() or manifest["target"] != target:
        raise ValueError("release manifest identifies a different version/target")
    archive = output / f"grapharch-{target}.zip"
    digest = sha256(archive)
    if (
        manifest["archive_sha256"] != digest
        or (output / f"{archive.name}.sha256").read_text()
        != f"{digest}  {archive.name}\n"
    ):
        raise ValueError(f"checksum mismatch: {archive.name}")
    with zipfile.ZipFile(archive) as zipped:
        data = zipped.read(binary_name(item))
        if hashlib.sha256(data).hexdigest() != manifest["binary_sha256"]:
            raise ValueError("archive contains a different executable")
        for name, digest in manifest["notices"].items():
            if hashlib.sha256(zipped.read(f"licenses/{name}")).hexdigest() != digest:
                raise ValueError("archive license notice mismatch")
    native = output / "npm" / f"cli-{item['key']}" / "bin" / binary_name(item)
    if sha256(native) != manifest["binary_sha256"]:
        raise ValueError("npm platform package contains a different executable")
    for name, digest in manifest["notices"].items():
        if sha256(output / "npm" / f"cli-{item['key']}" / "licenses" / name) != digest:
            raise ValueError("npm license notice mismatch")
    if "npm_archive" in manifest:
        package = output / manifest["npm_archive"]
        if (
            package.name != manifest["npm_archive"]
            or sha256(package) != manifest["npm_sha256"]
        ):
            raise ValueError("npm package checksum mismatch")
        with tarfile.open(package, "r:gz") as packed:
            member = packed.extractfile(f"package/bin/{binary_name(item)}")
            if (
                member is None
                or hashlib.sha256(member.read(MAX_BINARY + 1)).hexdigest()
                != manifest["binary_sha256"]
            ):
                raise ValueError("packed npm package contains a different executable")
            for name, digest in manifest["notices"].items():
                notice = packed.extractfile(f"package/licenses/{name}")
                if (
                    notice is None
                    or hashlib.sha256(notice.read()).hexdigest() != digest
                ):
                    raise ValueError("packed npm license notice mismatch")
    if "wheel" in manifest:
        wheel = output / manifest["wheel"]
        if wheel.name != manifest["wheel"] or sha256(wheel) != manifest["wheel_sha256"]:
            raise ValueError("wheel checksum mismatch")
        with zipfile.ZipFile(wheel) as packed:
            for name, digest in manifest["notices"].items():
                matches = [
                    path
                    for path in packed.namelist()
                    if ".dist-info/licenses/" in path and path.endswith(f"/{name}")
                ]
                if (
                    len(matches) != 1
                    or hashlib.sha256(packed.read(matches[0])).hexdigest() != digest
                ):
                    raise ValueError(
                        f"Python wheel omits or changes embedded asset notice: {name}"
                    )
        with tempfile.TemporaryDirectory() as temporary:
            native = extract_binary(wheel, item, Path(temporary) / binary_name(item))
            if sha256(native) != manifest["binary_sha256"]:
                raise ValueError("Python wheel contains a different executable")


def build_main(output: Path) -> None:
    check()
    package = output / "npm" / "grapharch"
    package.mkdir(parents=True, exist_ok=False)
    for name in ["bin", "lib"]:
        shutil.copytree(ROOT / "npm" / name, package / name)
    for name in ["package.json", "README.md", "LICENSE"]:
        shutil.copyfile(ROOT / "npm" / name, package / name)


def npm_command() -> list[str]:
    npm = shutil.which("npm")
    node = shutil.which("node")
    if npm is None or node is None:
        raise ValueError("Node.js and npm are required for package verification")
    if os.name == "nt":
        script = Path(npm).parent / "node_modules/npm/bin/npm-cli.js"
        if not script.is_file():
            raise ValueError(
                "npm installation does not provide its standard npm-cli.js entry point"
            )
    else:
        script = Path(npm).resolve()
    return [node, str(script)]


def pack_npm(output: Path, target: str | None) -> None:
    if target:
        item = platform(target)
        verify(output, target)
        directory = output / "npm" / f"cli-{item['key']}"
    else:
        directory = output / "npm" / "grapharch"
    result = subprocess.run(
        npm_command()
        + [
            "pack",
            "--ignore-scripts",
            "--json",
            "--pack-destination",
            str(output.resolve()),
        ],
        cwd=directory,
        check=True,
        capture_output=True,
        text=True,
    )
    records = json.loads(result.stdout)
    if len(records) != 1:
        raise ValueError("npm pack must produce one artifact")
    name = records[0]["filename"]
    if Path(name).name != name:
        raise ValueError("invalid npm package filename")
    if target:
        path = output / f"grapharch-{target}.json"
        manifest = json.loads(path.read_text())
        manifest.update(npm_archive=name, npm_sha256=sha256(output / name))
        path.write_text(json.dumps(manifest, indent=2) + "\n")
        verify(output, target)
    else:
        (output / "grapharch-npm.json").write_text(
            json.dumps(
                {
                    "version": version(),
                    "archive": name,
                    "sha256": sha256(output / name),
                },
                indent=2,
            )
            + "\n"
        )


def verify_release(output: Path) -> None:
    for item in PLATFORMS:
        verify(output, item["target"])
        manifest = json.loads((output / f"grapharch-{item['target']}.json").read_text())
        if "wheel" not in manifest or "npm_archive" not in manifest:
            raise ValueError(f"incomplete Python/npm release for {item['target']}")
    main = json.loads((output / "grapharch-npm.json").read_text())
    if (
        main["version"] != version()
        or main["archive"] != f"grapharch-{version()}.tgz"
        or sha256(output / main["archive"]) != main["sha256"]
    ):
        raise ValueError("main npm package metadata/checksum mismatch")
    if (
        len(list(output.glob("*.whl"))) != len(PLATFORMS)
        or len(list(output.glob("*.tgz"))) != len(PLATFORMS) + 1
    ):
        raise ValueError(
            "release must contain exactly the declared Python/npm packages"
        )
    with tarfile.open(output / main["archive"], "r:gz") as packed:
        metadata = packed.extractfile("package/package.json")
        if metadata is None or json.load(metadata) != json.loads(
            (ROOT / "npm/package.json").read_text()
        ):
            raise ValueError(
                "main npm package differs from the reviewed source manifest"
            )


def homebrew(output: Path, destination: Path) -> None:
    manifests = {}
    for item in PLATFORMS:
        if item["os"] == "win32":
            continue
        verify(output, item["target"])
        manifests[item["key"]] = json.loads(
            (output / f"grapharch-{item['target']}.json").read_text()
        )
    lines = [
        "class Grapharch < Formula",
        '  desc "Publish documentation websites, specifications, and PDFs from a knowledge graph"',
        '  homepage "https://ekgf.github.io/grapharch/"',
        f'  version "{version()}"',
        '  license "MIT"',
        "",
    ]
    for os_name, stanza in [("darwin", "macos"), ("linux", "linux")]:
        lines += [f"  on_{stanza} do"]
        for arch, selector in [("arm64", "arm"), ("x64", "intel")]:
            manifest = manifests[f"{os_name}-{arch}"]
            lines += [
                f"    on_{selector} do",
                f'      url "{REPOSITORY}/releases/download/v{version()}/{manifest["archive"]}"',
                f'      sha256 "{manifest["archive_sha256"]}"',
                "    end",
            ]
        lines += ["  end", ""]
    lines += [
        "  def install",
        '    bin.install "grapharch"',
        "  end",
        "",
        "  test do",
        '    assert_match version.to_s, shell_output("#{bin}/grapharch --version")',
        "  end",
        "end",
        "",
    ]
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text("\n".join(lines))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("check")
    commands.add_parser("sync-version")
    commands.add_parser("matrix")
    command = commands.add_parser("build")
    source = command.add_mutually_exclusive_group(required=True)
    source.add_argument("--binary", type=Path)
    source.add_argument("--wheel", type=Path)
    command.add_argument("--target", required=True)
    command.add_argument("--output", type=Path, required=True)
    command = commands.add_parser("verify")
    command.add_argument("--target", required=True)
    command.add_argument("--output", type=Path, required=True)
    command = commands.add_parser("build-main")
    command.add_argument("--output", type=Path, required=True)
    command = commands.add_parser("pack-npm")
    command.add_argument("--output", type=Path, required=True)
    command.add_argument("--target")
    command = commands.add_parser("verify-release")
    command.add_argument("--output", type=Path, required=True)
    command = commands.add_parser("homebrew")
    command.add_argument("--output", type=Path, required=True)
    command.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "check":
        check()
    elif args.command == "sync-version":
        sync_version()
    elif args.command == "matrix":
        print(json.dumps({"include": PLATFORMS}))
    elif args.command == "build":
        build(args.binary, args.wheel, args.target, args.output)
    elif args.command == "verify":
        verify(args.output, args.target)
    elif args.command == "build-main":
        build_main(args.output)
    elif args.command == "pack-npm":
        pack_npm(args.output, args.target)
    elif args.command == "verify-release":
        verify_release(args.output)
    elif args.command == "homebrew":
        homebrew(args.output, args.destination)


if __name__ == "__main__":
    try:
        main()
    except (
        ValueError,
        OSError,
        KeyError,
        zipfile.BadZipFile,
        subprocess.CalledProcessError,
    ) as error:
        print(f"GraphArch distribution: {error}", file=sys.stderr)
        sys.exit(1)
