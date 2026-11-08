import hashlib
import importlib.util
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "setup_plantuml", Path(__file__).parents[1] / "setup_plantuml.py"
)
setup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(setup)


class PlantUmlSetupTest(unittest.TestCase):
    def test_cache_is_probed_before_exporting_path(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            java = root / "java" / "bin" / "java"
            java.parent.mkdir(parents=True)
            java.touch()
            digest = hashlib.sha256(b"verified").hexdigest()
            cache = root / "cache" / "grapharch-plantuml" / setup.VERSION / digest
            cache.mkdir(parents=True)
            (cache / "plantuml.jar").write_bytes(b"verified")
            exported = root / "github-path"
            environment = {"JAVA_HOME": str(java.parent.parent), "RUNNER_TOOL_CACHE": str(root / "cache"), "GITHUB_PATH": str(exported)}
            with patch.dict(os.environ, environment), patch.object(setup, "SHA256", digest), patch.object(setup.platform, "system", return_value="Linux"), patch.object(setup.urllib.request, "urlopen") as download, patch.object(setup.subprocess, "run", side_effect=[SimpleNamespace(stdout=f"@startuml\n{setup.VERSION}\n@enduml\n"), SimpleNamespace(stdout="<svg>Parent Child</svg>")]) as command:
                setup.install()
                download.assert_not_called()
                self.assertEqual(command.call_args_list[0].args[0][1:], ["-preproc", "-pipe"])
                self.assertIn("layout smetana", command.call_args.kwargs["input"])
                self.assertEqual(exported.read_text().strip(), str(cache))
            exported.unlink()
            with patch.dict(os.environ, environment), patch.object(setup, "SHA256", digest), patch.object(setup.platform, "system", return_value="Linux"), patch.object(setup.subprocess, "run", return_value=SimpleNamespace(stdout="wrong version")):
                with self.assertRaisesRegex(ValueError, "unexpected version"):
                    setup.install()
                self.assertFalse(exported.exists())

    def test_failed_probe_reports_bounded_diagnostics(self):
        failure = subprocess.CalledProcessError(16, ["plantuml"], output="a" * 10000, stderr="missing layout engine")
        with patch.object(setup.subprocess, "run", side_effect=failure):
            with self.assertRaisesRegex(ValueError, "missing layout engine") as caught:
                setup.probe(Path("plantuml"), [], "", {})
        self.assertLess(len(str(caught.exception)), 4300)

    def test_verified_cache_is_reused_but_corruption_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            jar = Path(directory) / "plantuml.jar"
            jar.write_bytes(b"verified")
            with patch.object(setup, "SHA256", hashlib.sha256(b"verified").hexdigest()):
                setup.verify(jar)
                jar.write_bytes(b"corrupted")
                with self.assertRaisesRegex(ValueError, "checksum"):
                    setup.verify(jar)

    def test_launchers_quote_paths_and_preserve_arguments(self):
        with tempfile.TemporaryDirectory(prefix="java space ") as directory:
            root = Path(directory)
            posix = setup.launcher(root, root / "java", root / "plantuml.jar", False)
            self.assertIn('"$@"', posix.read_text())
            self.assertIn("-Djava.awt.headless=true", posix.read_text())
            windows = setup.launcher(root, root / "java.exe", root / "plantuml.jar", True)
            self.assertIn('" %*', windows.read_text())
            self.assertTrue(windows.name.endswith(".cmd"))

    def test_unknown_platform_and_unsafe_launcher_path_fail(self):
        with self.assertRaisesRegex(ValueError, "platform"):
            setup.windows_platform("Plan9")
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "unsafe"):
                setup.launcher(Path(directory), Path('bad%JAVA%'), Path('a.jar'), True)
