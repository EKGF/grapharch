"""Source installs execute only the explicitly pinned GraphArch checkout."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
import os
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    'source_action', Path(__file__).parents[1] / 'source_action.py')
source = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(source)


class SourceActionTests(unittest.TestCase):
    def test_revision_mismatch_and_dirty_inputs_fail_before_build(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(['git', 'init', '-q', str(root)], check=True)
            (root / 'Cargo.toml').write_text('[package]\nversion="0.1.0"\n')
            subprocess.run(['git', '-C', str(root), 'add', '.'], check=True)
            subprocess.run(['git', '-C', str(root), '-c', 'user.name=Fixture',
                            '-c', 'user.email=fixture@example.org', 'commit', '-qm', 'fixture'], check=True)
            revision = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
            source.validate_checkout(root, revision)
            with self.assertRaisesRegex(ValueError, 'revision'):
                source.validate_checkout(root, 'a' * 40)
            (root / 'Cargo.toml').write_text('modified')
            with self.assertRaisesRegex(ValueError, 'clean'):
                source.validate_checkout(root, revision)

    def test_source_build_uses_locked_manifest_pinned_toolchain_and_reports_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.toml').write_text('[package]\nversion="0.1.0"\n')
            (root / 'rust-toolchain.toml').write_text('[toolchain]\nchannel="nightly-2026-10-30"\n')
            environment = {'GRAPHARCH_BUILD_CONTEXT':'unrelated consumer build', 'RUNNER_TEMP': directory, 'GITHUB_PATH': str(root / 'paths'),
                           'GITHUB_OUTPUT': str(root / 'outputs')}
            with patch.dict(os.environ, environment), patch.object(source, 'validate_checkout'), \
                 patch.object(source.platform, 'system', return_value='Linux'), \
                 patch.object(source.subprocess, 'run') as run, \
                 patch.object(source.subprocess, 'check_output', side_effect=['grapharch 0.1.0\n','grapharch 0.1.0 (source aaaaaaaaaaaa)\nsource commit ' + 'a'*40]):
                source.install(root, 'a' * 40)
            command = run.call_args_list[-1]
            self.assertEqual(command.args[0][:7],
                             ['cargo', '+nightly-2026-10-30', 'build', '--locked', '--release', '--bin', 'grapharch'])
            self.assertEqual(command.kwargs['cwd'], root)
            self.assertEqual(command.kwargs['env']['GRAPHARCH_BUILD_REVISION'], 'a' * 40)
            self.assertEqual(command.kwargs['env']['GRAPHARCH_BUILD_RELEASE_VERSION'], '')
            self.assertEqual(command.kwargs['env']['GRAPHARCH_BUILD_CONTEXT'], '')
            self.assertIn('revision=' + 'a' * 40, (root / 'outputs').read_text())
            self.assertIn('binary-path=', (root / 'outputs').read_text())

    def test_install_selectors_are_mutually_exclusive_and_exact(self):
        for version, revision in [('', ''), ('0.1.0', 'a' * 40), ('', 'main'), ('latest', '')]:
            with self.subTest(version=version, revision=revision), self.assertRaises(ValueError):
                source.validate_selection(version, revision)
        self.assertEqual(source.validate_selection('0.1.0', ''), 'release')
        self.assertEqual(source.validate_selection('', 'a' * 40), 'source')
