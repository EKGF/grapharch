"""Immutable upstream fixtures must survive Git's Windows checkout settings."""

import hashlib
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
REFERENCE = Path("examples/w3c-rdf12-n-triples")


class ReferenceCheckoutTests(unittest.TestCase):
    def test_windows_checkout_preserves_recorded_source_digests(self):
        # The showcase definitions record each copied input's digest.
        metadata = (ROOT / REFERENCE / "showcase.ttl").read_text()
        inputs = ["source.html", "w3c-logo.png", "w3c-logo.svg"]
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory) / "repository"
            checkout = Path(directory) / "checkout"
            repository.mkdir()
            checkout.mkdir()
            shutil.copyfile(ROOT / ".gitattributes", repository / ".gitattributes")
            (repository / REFERENCE).mkdir(parents=True)
            for name in inputs:
                shutil.copyfile(ROOT / REFERENCE / name, repository / REFERENCE / name)

            def git(*arguments):
                return subprocess.run(
                    ["git", *arguments], cwd=repository, check=True,
                    capture_output=True,
                )

            git("init", "--quiet")
            git("-c", "core.autocrlf=false", "add", ".")
            git("config", "core.autocrlf", "true")
            git("checkout-index", "--all", f"--prefix={checkout.as_posix()}/")
            for name in inputs:
                with self.subTest(file=name):
                    expected = re.search(
                        rf'ga:localPath "{re.escape(name)}" ;.*?'
                        r'ga:contentDigest "([0-9a-f]{64})"',
                        metadata,
                        re.DOTALL,
                    ).group(1)
                    actual = hashlib.sha256((checkout / REFERENCE / name).read_bytes()).hexdigest()
                    self.assertEqual(actual, expected)


if __name__ == "__main__":
    unittest.main()
