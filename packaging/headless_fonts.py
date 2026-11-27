"""Java font mapping for PlantUML on Linux runners without fontconfig.

Temurin bundles FreeType but asks the system fontconfig library for its
fonts. A rootless runner without fontconfig, or without any installed font,
fails with "Fontconfig head is null". Java then needs an explicit
fontconfig.properties mapping its logical fonts to font files. GraphArch
already bundles the metric-compatible Liberation fonts for its PDFs, so the
mapping uses those files: no download and no system change are needed.
"""

import hashlib
import shutil
import subprocess
from pathlib import Path

SOURCE = Path(__file__).resolve().parents[1] / "src" / "publication" / "render" / "fonts"

# Physical family of each Java logical font.
LOGICAL = {
    "serif": "Serif",
    "sansserif": "Sans",
    "monospaced": "Mono",
    "dialog": "Sans",
    "dialoginput": "Mono",
}
STYLES = {
    "plain": ("Regular", ""),
    "bold": ("Bold", " Bold"),
    "italic": ("Italic", " Italic"),
    "bolditalic": ("BoldItalic", " Bold Italic"),
}
UNSAFE = "\\\r\n"


class FontFile:
    """One bundled Liberation face with the name Java uses for it."""

    def __init__(self, family, style):
        file_style, name_style = STYLES[style]
        self.file = f"Liberation{family}-{file_style}.ttf"
        self.name = f"Liberation {family}{name_style}"

    def key(self):
        return "filename." + self.name.replace(" ", "_")


def fontconfig_missing(system, run=subprocess.run, which=shutil.which):
    """Whether Java on this platform cannot obtain fonts from fontconfig."""
    if system != "Linux":
        return False
    listing = which("fc-list")
    if listing is None:
        return True
    try:
        result = run([listing], capture_output=True, text=True, timeout=30, check=False)
    except (OSError, subprocess.TimeoutExpired):
        return True
    return result.returncode != 0 or not result.stdout.strip()


def faces():
    for family in sorted(set(LOGICAL.values())):
        for style in STYLES:
            yield FontFile(family, style)


def properties(directory):
    """The fontconfig.properties text mapping every logical font and style."""
    if any(char in str(directory) for char in UNSAFE):
        raise ValueError("unsafe font directory path")
    lines = ["version=1", "sequence.allfonts=latin-1"]
    for logical, family in LOGICAL.items():
        for style in STYLES:
            lines.append(f"{logical}.{style}.latin-1={FontFile(family, style).name}")
    for face in faces():
        lines.append(f"{face.key()}={(directory / face.file).as_posix()}")
    return "\n".join(lines) + "\n"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def install(root, source=SOURCE):
    """Copy the bundled faces under root and write their Java mapping.

    Returns the fontconfig.properties path for -Dsun.awt.fontconfig.
    """
    directory = root / "fonts"
    directory.mkdir(parents=True, exist_ok=True)
    for face in faces():
        origin = source / face.file
        if not origin.is_file():
            raise ValueError(f"Bundled font is missing: {origin}")
        target = directory / face.file
        if not target.is_file() or digest(target) != digest(origin):
            pending = directory / (face.file + ".copy")
            shutil.copyfile(origin, pending)
            pending.replace(target)
    mapping = directory / "fontconfig.properties"
    mapping.write_text(properties(directory), encoding="utf-8", newline="\n")
    return mapping
