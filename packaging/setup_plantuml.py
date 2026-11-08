"""Explicit CI setup for the pinned, local-only UML renderer (never runtime setup)."""

import hashlib
import os
import platform
import shlex
import subprocess
import urllib.request
from pathlib import Path

VERSION = "1.2026.9"
SHA256 = "835b2f2eb1d756da3f03bffdbd0baf252b4b858547a741ac8e972958a09c7f91"
URL = f"https://github.com/plantuml/plantuml/releases/download/v{VERSION}/plantuml-mit-{VERSION}.jar"
MAX_BYTES = 32 * 1024 * 1024


def windows_platform(name):
    if name not in ("Windows", "Linux", "Darwin"):
        raise ValueError(f"Unsupported PlantUML setup platform: {name}")
    return name == "Windows"


def verify(path):
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as source:
        while chunk := source.read(65536):
            size += len(chunk)
            if size > MAX_BYTES:
                raise ValueError("PlantUML jar exceeds 32 MiB")
            digest.update(chunk)
    if digest.hexdigest() != SHA256:
        raise ValueError("PlantUML jar checksum mismatch; remove the corrupt cache explicitly")


def launcher(root, java, jar, windows):
    for path in (java, jar):
        if any(char in str(path) for char in '\r\n"%!'):
            raise ValueError("unsafe PlantUML launcher path")
    path = root / ("plantuml.cmd" if windows else "plantuml")
    if windows:
        text = f'@echo off\r\n"{java}" -Djava.awt.headless=true -jar "{jar}" %*\r\nexit /b %errorlevel%\r\n'
    else:
        text = f'#!/bin/sh\nexec {shlex.quote(str(java))} -Djava.awt.headless=true -jar {shlex.quote(str(jar))} "$@"\n'
    path.write_text(text, encoding="utf-8", newline="")
    path.chmod(0o755)
    return path


def install():
    windows = windows_platform(platform.system())
    java = Path(os.environ["JAVA_HOME"]) / "bin" / ("java.exe" if windows else "java")
    if not java.is_file():
        raise ValueError(f"Java is not installed: {java}")
    root = Path(os.environ["RUNNER_TOOL_CACHE"]) / "grapharch-plantuml" / VERSION / SHA256
    root.mkdir(parents=True, exist_ok=True)
    jar = root / "plantuml.jar"
    if not jar.exists():
        pending = root / "plantuml.jar.download"
        try:
            with urllib.request.urlopen(URL, timeout=60) as response, pending.open("wb") as output:
                total = 0
                while chunk := response.read(65536):
                    total += len(chunk)
                    if total > MAX_BYTES:
                        raise ValueError("PlantUML download exceeds 32 MiB")
                    output.write(chunk)
            verify(pending)
            pending.replace(jar)
        finally:
            pending.unlink(missing_ok=True)
    verify(jar)
    command = launcher(root, java, jar, windows)
    version = subprocess.run([str(command), "-version"], check=True, capture_output=True, text=True, timeout=30)
    if not version.stdout.startswith(f"PlantUML version {VERSION} "):
        raise ValueError("Installed PlantUML reports an unexpected version")
    # Verify the actual MIT build contains the required in-process UML engine.
    environment = dict(os.environ, PLANTUML_SECURITY_PROFILE="SANDBOX")
    sample = "@startuml\n!pragma layout smetana\nclass Parent\nclass Child\nParent <|-- Child\n@enduml\n"
    rendered = subprocess.run([str(command), "-pipe", "-tsvg", "-nometadata"], input=sample, capture_output=True, text=True, check=True, timeout=30, env=environment)
    if "<svg" not in rendered.stdout or "Parent" not in rendered.stdout or "Child" not in rendered.stdout:
        raise ValueError("Pinned PlantUML failed the Smetana rendering probe")
    with Path(os.environ["GITHUB_PATH"]).open("a", encoding="utf-8") as output:
        output.write(str(root) + "\n")
    print(f"Verified local PlantUML MIT {VERSION} ({SHA256}); Smetana probe passed")


if __name__ == "__main__":
    install()
