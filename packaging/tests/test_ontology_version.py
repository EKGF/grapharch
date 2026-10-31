"""Ontology release metadata participates in the existing Cargo version sync."""

import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class OntologyVersionTests(unittest.TestCase):
    def fixture(self, root):
        for name in ["packaging/distribute.py", "packaging/platforms.json",
                     "npm/package.json", "pyproject.toml",
                     "docs/other/ontology/publication.ttl"]:
            destination = root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, destination)
        (root / "Cargo.toml").write_text('[package]\nversion = "0.2.7"\n')
        path = root / "npm/package.json"
        manifest = json.loads(path.read_text())
        manifest["version"] = "0.2.7"
        manifest["optionalDependencies"] = dict.fromkeys(manifest["optionalDependencies"], "0.2.7")
        path.write_text(json.dumps(manifest))
        ontology = root / "docs/other/ontology/publication.ttl"
        text = ontology.read_text()
        text = re.sub(r'owl:versionIRI <[^>]+>', 'owl:versionIRI <https://ekgf.org/ontology/grapharch/0.0.1>', text)
        text = re.sub(r'owl:versionInfo "[^"]+"', 'owl:versionInfo "0.0.1 — development draft"', text)
        ontology.write_text(text)
        return ontology

    def command(self, root, command):
        return subprocess.run([sys.executable, str(root / "packaging/distribute.py"), command],
                              capture_output=True, text=True, check=False)

    def test_check_rejects_ontology_drift_even_when_packages_match(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root)
            result = self.command(root, "check")
            self.assertNotEqual(result.returncode, 0, "ontology drift must block source verification")
            self.assertIn("ontology", result.stderr.lower())

    def test_sync_updates_only_ontology_version_metadata_from_cargo(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            ontology = self.fixture(root)
            before = ontology.read_text()
            result = self.command(root, "sync-version")
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(ontology.read_text(), before.replace(
                "<https://ekgf.org/ontology/grapharch/0.0.1>",
                "<https://ekgf.org/ontology/grapharch/0.2.7>"
            ).replace('owl:versionInfo "0.0.1 — development draft"',
                      'owl:versionInfo "0.2.7 — development draft"'))
            self.assertEqual(self.command(root, "check").returncode, 0)
            unchanged = ontology.read_bytes()
            self.assertEqual(self.command(root, "sync-version").returncode, 0)
            self.assertEqual(ontology.read_bytes(), unchanged)


if __name__ == "__main__":
    unittest.main()
