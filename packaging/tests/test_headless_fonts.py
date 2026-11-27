"""PlantUML setup on rootless runners without fontconfig or a downloaded JRE."""

import hashlib
import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

PACKAGING = Path(__file__).parents[1]
sys.path.insert(0, str(PACKAGING))
spec = importlib.util.spec_from_file_location("setup_plantuml", PACKAGING / "setup_plantuml.py")
setup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(setup)
fonts = setup.headless_fonts


def listing(stdout, returncode=0):
    return lambda *args, **kwargs: SimpleNamespace(stdout=stdout, returncode=returncode)


class FontconfigDetectionTest(unittest.TestCase):
    def test_only_linux_without_listed_fonts_needs_a_mapping(self):
        self.assertFalse(fonts.fontconfig_missing("Darwin", which=lambda name: None))
        self.assertFalse(fonts.fontconfig_missing("Windows", which=lambda name: None))
        self.assertTrue(fonts.fontconfig_missing("Linux", which=lambda name: None))
        present = lambda name: "/usr/bin/fc-list"
        self.assertTrue(fonts.fontconfig_missing("Linux", run=listing(""), which=present))
        self.assertTrue(fonts.fontconfig_missing("Linux", run=listing("x", 1), which=present))
        fonts_listed = listing("/usr/share/fonts/a.ttf: A:style=Regular\n")
        self.assertFalse(fonts.fontconfig_missing("Linux", run=fonts_listed, which=present))

    def test_failing_listing_counts_as_missing(self):
        def failure(*args, **kwargs):
            raise subprocess.TimeoutExpired("fc-list", 30)
        present = lambda name: "/usr/bin/fc-list"
        self.assertTrue(fonts.fontconfig_missing("Linux", run=failure, which=present))


class FontMappingTest(unittest.TestCase):
    def test_every_logical_font_and_style_maps_to_a_bundled_file(self):
        for face in fonts.faces():
            self.assertTrue((fonts.SOURCE / face.file).is_file(), face.file)
        with tempfile.TemporaryDirectory(prefix="font cache ") as directory:
            mapping = fonts.install(Path(directory))
            entries = dict(
                line.split("=", 1) for line in mapping.read_text(encoding="utf-8").splitlines()
            )
            self.assertEqual(entries["version"], "1")
            for logical in ["serif", "sansserif", "monospaced", "dialog", "dialoginput"]:
                for style in ["plain", "bold", "italic", "bolditalic"]:
                    name = entries[f"{logical}.{style}.latin-1"]
                    path = Path(entries["filename." + name.replace(" ", "_")])
                    self.assertTrue(path.is_file(), path)
                    self.assertTrue(path.is_relative_to(Path(directory)))
            self.assertEqual(entries["sansserif.plain.latin-1"], "Liberation Sans")
            copied = Path(directory) / "fonts" / "LiberationSans-Regular.ttf"
            original = fonts.SOURCE / "LiberationSans-Regular.ttf"
            self.assertEqual(copied.read_bytes(), original.read_bytes())
            copied.write_bytes(b"corrupt")
            fonts.install(Path(directory))
            self.assertEqual(copied.read_bytes(), original.read_bytes())

    def test_missing_bundled_font_and_unsafe_directory_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "missing"):
                fonts.install(Path(directory), source=Path(directory) / "absent")
        with self.assertRaisesRegex(ValueError, "unsafe"):
            fonts.properties(Path("bad\npath"))

    def test_launcher_passes_the_mapping_to_java(self):
        with tempfile.TemporaryDirectory(prefix="java space ") as directory:
            root = Path(directory)
            mapping = root / "fonts" / "fontconfig.properties"
            text = setup.launcher(root, root / "java", root / "plantuml.jar", False, mapping).read_text()
            self.assertIn(f"'-Dsun.awt.fontconfig={mapping}'", text)
            self.assertIn("-Djava.awt.headless=true", text)
            self.assertNotIn("sun.awt.fontconfig", setup.launcher(root, root / "java", root / "a.jar", False).read_text())
            with self.assertRaisesRegex(ValueError, "unsafe"):
                setup.launcher(root, root / "java", root / "a.jar", False, Path('bad"path'))


class FontlessInstallTest(unittest.TestCase):
    def test_fontless_linux_install_probes_with_the_mapping(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            java = root / "preinstalled" / "bin" / "java"
            java.parent.mkdir(parents=True)
            java.touch()
            digest = hashlib.sha256(b"verified").hexdigest()
            cache = root / "cache" / "grapharch-plantuml" / setup.VERSION / digest
            cache.mkdir(parents=True)
            (cache / "plantuml.jar").write_bytes(b"verified")
            exported, environment_file = root / "github-path", root / "github-env"
            environment = {
                "GRAPHARCH_JAVA_HOME": str(java.parent.parent),
                "JAVA_HOME": str(root / "downloaded"),
                "RUNNER_TOOL_CACHE": str(root / "cache"),
                "GITHUB_PATH": str(exported),
                "GITHUB_ENV": str(environment_file),
            }
            outputs = [SimpleNamespace(stdout=f"@startuml\n{setup.VERSION}\n@enduml\n"),
                       SimpleNamespace(stdout="<svg>Parent Child</svg>")]
            with patch.dict(os.environ, environment, clear=True), \
                    patch.object(setup, "SHA256", digest), \
                    patch.object(setup.platform, "system", return_value="Linux"), \
                    patch.object(fonts, "fontconfig_missing", return_value=True), \
                    patch.object(setup.subprocess, "run", side_effect=outputs) as command:
                setup.install()
            self.assertTrue((cache / "fonts" / "fontconfig.properties").is_file())
            self.assertIn("-Dsun.awt.fontconfig=", (cache / "plantuml").read_text())
            self.assertIn(str(java), (cache / "plantuml").read_text())
            self.assertEqual(command.call_args.kwargs["env"]["JAVA_HOME"], str(java.parent.parent))
            self.assertEqual(environment_file.read_text(), f"JAVA_HOME={java.parent.parent}\n")

    def test_java_home_precedence_and_absence(self):
        self.assertEqual(setup.java_home({"GRAPHARCH_JAVA_HOME": "/a", "JAVA_HOME": "/b"}), Path("/a"))
        self.assertEqual(setup.java_home({"GRAPHARCH_JAVA_HOME": "", "JAVA_HOME": "/b"}), Path("/b"))
        with self.assertRaisesRegex(ValueError, "GRAPHARCH_JAVA_HOME"):
            setup.java_home({})


class SelectionTest(unittest.TestCase):
    def test_inputs_are_validated_before_installation(self):
        absolute = str(Path(tempfile.gettempdir()).resolve())
        self.assertEqual(setup.selection({"GRAPHARCH_UML_RENDERER": "true"}), (True, False))
        self.assertEqual(setup.selection({"GRAPHARCH_UML_RENDERER": "false"}), (False, False))
        self.assertEqual(
            setup.selection({"GRAPHARCH_UML_RENDERER": "true", "GRAPHARCH_JAVA_HOME": absolute}),
            (True, True),
        )
        for value in ["", "yes", "False"]:
            with self.assertRaisesRegex(ValueError, "uml-renderer"):
                setup.selection({"GRAPHARCH_UML_RENDERER": value})
        for home in ["relative/jre", absolute + "\nx"]:
            with self.assertRaisesRegex(ValueError, "java-home"):
                setup.selection({"GRAPHARCH_UML_RENDERER": "true", "GRAPHARCH_JAVA_HOME": home})
        with self.assertRaisesRegex(ValueError, "requires uml-renderer"):
            setup.selection({"GRAPHARCH_UML_RENDERER": "false", "GRAPHARCH_JAVA_HOME": absolute})

    def test_select_writes_step_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            with patch.dict(os.environ, {"GRAPHARCH_UML_RENDERER": "false", "GITHUB_OUTPUT": str(output)}, clear=True):
                setup.select()
            self.assertEqual(output.read_text(), "install=false\njava=temurin\n")


if __name__ == "__main__":
    unittest.main()
