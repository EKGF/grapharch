"""The shipped packages contain and execute the same CLI artifact."""

import json
import re
import subprocess
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class DistributionTests(unittest.TestCase):
    def test_source_verification_uses_the_declared_linux_release_host(self):
        platforms = json.loads((ROOT / "packaging/platforms.json").read_text())
        host = next(item["runner"] for item in platforms
                    if item["target"] == "x86_64-unknown-linux-gnu")
        workflow = (ROOT / ".github/workflows/distribute.yml").read_text()
        source_job = workflow.split("  verify-source:\n", 1)[1].split("  binaries:\n", 1)[0]
        runner = re.search(r"^    runs-on: (.+)$", source_job, re.MULTILINE).group(1)
        self.assertEqual(runner, host)

    def test_builds_platform_package_and_verified_standalone_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            work = Path(temporary)
            binary = work / "grapharch"
            binary.write_bytes(
                b"\xcf\xfa\xed\xfe\x0c\x00\x00\x01" + b"native-cli-fixture\n"
            )
            result = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "packaging/distribute.py"),
                    "build",
                    "--binary",
                    str(binary),
                    "--target",
                    "aarch64-apple-darwin",
                    "--output",
                    str(work / "dist"),
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            package = json.loads(
                (work / "dist/npm/cli-darwin-arm64/package.json").read_text()
            )
            self.assertEqual(package["name"], "@grapharch/cli-darwin-arm64")
            self.assertEqual(package["cpu"], ["arm64"])
            self.assertEqual(
                (work / "dist/npm/cli-darwin-arm64/bin/grapharch").read_bytes(),
                binary.read_bytes(),
            )
            archive = work / "dist/grapharch-aarch64-apple-darwin.zip"
            with zipfile.ZipFile(archive) as zipped:
                self.assertEqual(zipped.read("grapharch"), binary.read_bytes())
                self.assertTrue(
                    (zipped.getinfo("grapharch").external_attr >> 16) & 0o111
                )
            checked = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "packaging/distribute.py"),
                    "verify",
                    "--output",
                    str(work / "dist"),
                    "--target",
                    "aarch64-apple-darwin",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(checked.returncode, 0, checked.stderr)
            archive.write_bytes(b"corrupt")
            checked = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "packaging/distribute.py"),
                    "verify",
                    "--output",
                    str(work / "dist"),
                    "--target",
                    "aarch64-apple-darwin",
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(
                checked.returncode,
                0,
                "corrupt release artifacts must fail verification",
            )

    def test_rejects_unknown_platform_and_version_drift(self):
        with tempfile.TemporaryDirectory() as temporary:
            result = subprocess.run(
                [
                    sys.executable,
                    str(ROOT / "packaging/distribute.py"),
                    "build",
                    "--binary",
                    __file__,
                    "--target",
                    "unknown",
                    "--output",
                    temporary,
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("unsupported target", result.stderr)

    def test_version_matches_main_package(self):
        result = subprocess.run(
            [sys.executable, str(ROOT / "packaging/distribute.py"), "check"],
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
