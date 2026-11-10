"""A successful candidate cannot authorize arbitrary edits in release metadata files."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))
from auto_release import FILES, verify_release

class ReleaseProof(unittest.TestCase):
    def command(self, *args):
        return subprocess.check_output(['git', *args], text=True).strip()

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.previous = Path.cwd()
        self.addCleanup(self.temporary.cleanup)
        self.addCleanup(os.chdir, self.previous)
        os.chdir(self.temporary.name)
        self.command('init', '-q')
        self.command('config', 'user.name', 'Release regression')
        self.command('config', 'user.email', 'test@example.invalid')
        self.old = '0.2.2'
        self.new = '0.2.3'
        Path('Cargo.toml').write_text(f'[package]\nname = "grapharch"\nversion = "{self.old}"\n')
        Path('Cargo.lock').write_text(f'[[package]]\nname = "grapharch"\nversion = "{self.old}"\n')
        Path('npm').mkdir()
        Path('npm/package.json').write_text(json.dumps({'version': self.old, 'optionalDependencies': {'grapharch-test': self.old}}))
        Path('docs/other/ontology').mkdir(parents=True)
        Path('docs/other/ontology/publication.ttl').write_text(
            f'<urn:ontology> owl:versionIRI <https://ekgf.org/ontology/grapharch/{self.old}> ;\n'
            f'owl:versionInfo "{self.old} draft" .\n')
        self.command('add', '.')
        self.command('commit', '-qm', 'test(source): reviewed candidate')
        self.source = self.command('rev-parse', 'HEAD')

    def release(self, *, extra=None, version=None):
        new = version or self.new
        for name in FILES:
            path = Path(name)
            path.write_text(path.read_text().replace(self.old, new))
        if extra:
            path, text = extra
            with Path(path).open('a') as stream:
                stream.write(text)
        self.command('add', '.')
        self.command('commit', '-qm', f'chore(release): publish {new}\n\nGraphArch-Release-Source: {self.source}')
        return self.command('rev-parse', 'HEAD')

    def verify(self, revision):
        with patch('auto_release.verify_proof') as proof:
            verify_release('EKGF/grapharch', 42, revision, self.source)
            proof.assert_called_once_with('EKGF/grapharch', 42, self.source)

    def test_exact_metadata_child_preserves_validated_code(self):
        self.verify(self.release())

    def test_allowed_filename_does_not_authorize_dependency_edits(self):
        revision = self.release(extra=('Cargo.toml', '\n[dependencies]\nunreviewed = "1"\n'))
        with self.assertRaisesRegex(ValueError, 'non-version content'):
            self.verify(revision)

    def test_extra_source_file_is_not_a_validated_release(self):
        revision = self.release(extra=('source.rs', 'fn unreviewed() {}\n'))
        with self.assertRaisesRegex(ValueError, 'beyond version metadata'):
            self.verify(revision)

    def test_major_bump_cannot_use_automatic_release_proof(self):
        revision = self.release(version='1.0.0')
        with self.assertRaisesRegex(ValueError, 'automatically change major'):
            self.verify(revision)
