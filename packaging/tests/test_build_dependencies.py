"""A stalled package mirror must not occupy a release runner indefinitely."""

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch


SPEC = importlib.util.spec_from_file_location(
    "install_build_dependencies",
    Path(__file__).resolve().parents[1] / "install_build_dependencies.py",
)
INSTALLER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INSTALLER)


class BuildDependencyTests(unittest.TestCase):
    def test_stalled_package_manager_is_terminated(self):
        with tempfile.TemporaryDirectory() as directory:
            script = Path(directory) / "stalled_apt.py"
            script.write_text("import time\ntime.sleep(60)\n")
            start = time.monotonic()
            with self.assertRaises(subprocess.TimeoutExpired):
                INSTALLER.install(apt=(sys.executable, str(script)), timeout=0.1)
            self.assertLess(time.monotonic() - start, 3)

    def test_update_failure_prevents_installation(self):
        failure = subprocess.CalledProcessError(100, ["apt-get", "update"])
        with patch.object(INSTALLER.subprocess, "run", side_effect=failure) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                INSTALLER.install()
            run.assert_called_once()
            self.assertEqual(run.call_args.args[0][-1], "update")

    def test_transient_mirror_failures_are_retried_within_a_bound(self):
        with patch.object(INSTALLER.subprocess, "run") as run:
            INSTALLER.install()
        for call in run.call_args_list:
            command = call.args[0]
            self.assertIn("Acquire::Retries=5", command)
            self.assertIn("Acquire::http::Timeout=60", command)
            self.assertIsNotNone(call.kwargs["timeout"])
        install = run.call_args_list[-1].args[0]
        self.assertIn("--no-install-recommends", install)


if __name__ == "__main__":
    unittest.main()
