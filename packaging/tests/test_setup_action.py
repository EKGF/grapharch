import importlib.util
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "setup_action", Path(__file__).parents[1] / "setup_action.py"
)
setup = importlib.util.module_from_spec(spec)
spec.loader.exec_module(setup)


class SetupActionTest(unittest.TestCase):
    def test_platform_contract(self):
        with (
            patch.object(setup.platform, "system", return_value="Linux"),
            patch.object(setup.platform, "machine", return_value="aarch64"),
            patch.object(setup.platform, "libc_ver", return_value=("glibc", "2.28")),
        ):
            self.assertEqual(setup.target(), "aarch64-unknown-linux-gnu")
        with (
            patch.object(setup.platform, "system", return_value="Linux"),
            patch.object(setup.platform, "machine", return_value="x86_64"),
            patch.object(setup.platform, "libc_ver", return_value=("musl", "1.2")),
            self.assertRaisesRegex(ValueError, "glibc"),
        ):
            setup.target()
        with (
            patch.object(setup.platform, "system", return_value="Plan9"),
            self.assertRaisesRegex(ValueError, "Unsupported"),
        ):
            setup.target()

    def test_corrupt_cached_archive_fails_before_execution(self):
        import io
        import os
        import tempfile

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            target = "aarch64-apple-darwin"
            directory = root / "grapharch" / "0.0.1" / target
            directory.mkdir(parents=True)
            name = f"grapharch-{target}.zip"
            (directory / name).write_bytes(b"corrupted cache")
            environment = {
                "GRAPHARCH_VERSION": "0.0.1",
                "RUNNER_TOOL_CACHE": temporary,
                "GITHUB_OUTPUT": str(root / "outputs"),
                "GITHUB_PATH": str(root / "paths"),
            }
            with (
                patch.dict(os.environ, environment),
                patch.object(setup, "target", return_value=target),
                patch.object(setup.sys, "argv", ["setup_action.py", "install"]),
                patch.object(
                    setup.urllib.request,
                    "urlopen",
                    return_value=io.BytesIO(f"{'0' * 64}  {name}\n".encode()),
                ),
                patch.object(setup.subprocess, "run") as execute,
                self.assertRaisesRegex(ValueError, "checksum mismatch"),
            ):
                setup.main()
            execute.assert_not_called()
            self.assertFalse((root / "paths").exists())
