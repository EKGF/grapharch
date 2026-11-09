"""Exercise release reservation against a real isolated Git remote."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))
from auto_release import FILES, prepare

ROOT = Path(__file__).resolve().parents[2]


class ReleaseReservation(unittest.TestCase):
    def git(self, *args):
        return subprocess.check_output(['git', *args], text=True, stderr=subprocess.PIPE).strip()

    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        previous = Path.cwd()
        self.addCleanup(os.chdir, previous)
        root = Path(self.temporary.name)
        self.remote = root / 'origin.git'
        self.checkout = root / 'checkout'
        self.output = root / 'outputs'
        self.git('init', '--bare', str(self.remote))
        self.git('init', '--initial-branch=main', str(self.checkout))
        os.chdir(self.checkout)
        self.git('config', 'user.name', 'Release regression')
        self.git('config', 'user.email', 'test@example.invalid')
        # bounded: only the seven explicit version/check inputs, no repository corpus.
        for name in (*FILES, 'pyproject.toml', 'packaging/distribute.py', 'packaging/platforms.json'):
            destination = Path(name)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, destination)
        self.old = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
        major, minor, patch_number = map(int, self.old.split('.'))
        self.new = f'{major}.{minor}.{patch_number + 1}'
        self.git('add', '.')
        self.git('commit', '-m', 'test(source): original candidate')
        self.source = self.git('rev-parse', 'HEAD')
        self.git('tag', f'v{self.old}')
        self.git('remote', 'add', 'origin', str(self.remote))
        self.git('push', 'origin', 'main', '--tags')

    def reserve(self):
        with patch('auto_release.api', return_value={'head_sha': self.source}), patch('auto_release.verify_proof') as proof:
            prepare('EKGF/grapharch', 42, self.output, 'auto')
            proof.assert_called_once_with('EKGF/grapharch', 42, self.source)

    def remote_main(self):
        return self.git('--git-dir', str(self.remote), 'rev-parse', 'refs/heads/main')

    def test_reservation_is_metadata_only_and_retry_does_not_bump_twice(self):
        self.reserve()
        release = self.remote_main()
        self.assertEqual(self.git('rev-parse', release + '^'), self.source)
        self.assertEqual(set(self.git('diff', '--name-only', self.source, release).splitlines()), set(FILES))
        self.assertEqual(self.git('rev-parse', f'v{self.new}^{{commit}}'), release)
        self.assertIn(f'version={self.new}', self.output.read_text())
        self.reserve()
        self.assertEqual(self.remote_main(), release)
        self.assertEqual(len(self.git('tag', '--list', f'v{self.new}').splitlines()), 1)

    def test_superseded_source_does_not_reserve_a_version(self):
        Path('newer.txt').write_text('new source\n')
        self.git('add', 'newer.txt')
        self.git('commit', '-m', 'test(source): newer candidate')
        self.git('push', 'origin', 'main')
        newer = self.remote_main()
        self.reserve()
        self.assertEqual(self.remote_main(), newer)
        self.assertFalse(self.output.exists())
        self.assertEqual(self.git('tag', '--list', f'v{self.new}'), '')

    def test_resuming_reserved_source_never_rewinds_newer_main(self):
        self.reserve()
        release = self.remote_main()
        Path('newer.txt').write_text('next source\n')
        self.git('add', 'newer.txt')
        self.git('commit', '-m', 'test(source): next candidate')
        self.git('push', 'origin', 'HEAD:refs/heads/main')
        newer = self.remote_main()
        self.reserve()
        self.assertEqual(self.remote_main(), newer)
        self.assertIn(f'revision={release}', self.output.read_text())
