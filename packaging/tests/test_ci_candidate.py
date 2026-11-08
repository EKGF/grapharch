import importlib.util
import json
import struct
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))
import ci_candidate as candidate

REVISION = 'a' * 40
IDENTITY = {'revision': REVISION, 'run_id': '123', 'target': 'x86_64-unknown-linux-gnu', 'os': 'Linux', 'arch': 'x86_64', 'profile': 'test', 'features': 'all', 'generator_build': None}


class CandidateTests(unittest.TestCase):
    def test_consumer_retry_preserves_sealed_producer_build(self):
        from build_context import BuildContext
        environment = {'GITHUB_ACTIONS':'true', 'GITHUB_SHA':REVISION,
            'GITHUB_REPOSITORY':'EKGF/grapharch',
            'GITHUB_WORKFLOW_REF':'EKGF/grapharch/.github/workflows/pages.yml@refs/heads/main',
            'GITHUB_RUN_ID':'123', 'GITHUB_RUN_NUMBER':'7', 'GITHUB_RUN_ATTEMPT':'1'}
        with tempfile.TemporaryDirectory() as temporary, patch.dict(candidate.os.environ, environment, clear=True):
            producer = json.loads(BuildContext.current(REVISION).json())
            identity = dict(IDENTITY, generator_build=producer)
            root, engine, _, messages, manifest = self.fixture(temporary)
            with patch.object(candidate, 'identity', return_value=identity):
                candidate.record(root, REVISION, messages, manifest, True)
            with patch.dict(candidate.os.environ, {'GITHUB_RUN_ATTEMPT':'2'}), patch.object(candidate, 'identity', return_value=dict(IDENTITY)):
                self.assertEqual(candidate.verify(root, REVISION, manifest, 'engine'), engine)
                self.assertEqual(candidate.load(root, REVISION, manifest)['identity']['generator_build'], producer)
            with patch.dict(candidate.os.environ, {'GITHUB_RUN_ID':'999'}), patch.object(candidate, 'identity', return_value=dict(IDENTITY)):
                with self.assertRaisesRegex(ValueError, 'another build'):
                    candidate.verify(root, REVISION, manifest, 'engine')

    def test_rejects_generator_revision_from_another_checkout(self):
        with patch.dict(candidate.os.environ, {'GRAPHARCH_BUILD_REVISION':'b'*40}), patch.object(candidate,'validate_checkout'):
            with self.assertRaisesRegex(ValueError,'generator identity'):
                candidate.identity(Path.cwd(),REVISION)

    def fixture(self, directory):
        root = Path(directory).resolve()
        target = root / 'target/debug with spaces'
        target.mkdir(parents=True)
        engine = target / 'grapharch'
        engine.write_bytes(b'\x7fELF\x02\x01' + b'\0' * 12 + struct.pack('<H', 62) + b'candidate engine')
        native = target / 'persistent_family'
        native.write_bytes(b'\x7fELF\x02\x01' + b'\0' * 12 + struct.pack('<H', 62) + b'compiled test harness')
        messages = [{'reason': 'compiler-artifact', 'target': {'name': name, 'kind': [kind]},
                     'profile': {'test': test, 'opt_level': '0'}, 'features': [], 'executable': str(path)}
                    for name, kind, test, path in [('grapharch', 'bin', False, engine),
                        ('grapharch', 'bin', True, native), ('persistent_family', 'test', True, native)]]
        manifest = root / 'output/ci-engine.json'
        return root, engine, native, messages, manifest

    def test_reuses_exact_cli_and_harness_with_space_safe_arguments(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(candidate, 'identity', return_value=IDENTITY):
            root, engine, native, messages, manifest = self.fixture(temporary)
            candidate.record(root, REVISION, messages, manifest, True)
            self.assertEqual(candidate.verify(root, REVISION, manifest, 'engine'), engine)
            self.assertEqual(candidate.verify(root, REVISION, manifest, 'native-store'), native)
            with patch.object(candidate.subprocess, 'run') as run:
                candidate.execute(root, REVISION, manifest, 'engine', ['publication', 'a path', '$(literal)'])
                self.assertEqual(run.call_args.args[0], [str(engine), 'publication', 'a path', '$(literal)'])
                self.assertNotIn('shell', run.call_args.kwargs)

    def test_rejects_corruption_and_wrong_build_identity_without_rebuild(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(candidate, 'identity', return_value=IDENTITY):
            root, engine, _, messages, manifest = self.fixture(temporary)
            candidate.record(root, REVISION, messages, manifest, True)
            for key, value in [('revision', 'b' * 40), ('os', 'Windows'), ('arch', 'aarch64'),
                               ('profile', 'release'), ('features', 'default'), ('run_id', '456')]:
                with self.subTest(key=key), patch.object(candidate, 'identity', return_value=dict(IDENTITY, **{key: value})), self.assertRaisesRegex(ValueError, 'changed'):
                    candidate.verify(root, REVISION, manifest, 'engine')
            engine.write_bytes(b'corrupt')
            with self.assertRaisesRegex(ValueError, 'checksum'):
                candidate.verify(root, REVISION, manifest, 'engine')

    def test_rejects_manifest_tampering_missing_harness_and_escaped_target(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(candidate, 'identity', return_value=IDENTITY):
            root, engine, _, messages, manifest = self.fixture(temporary)
            with self.assertRaisesRegex(ValueError, 'required executables'):
                candidate.record(root, REVISION, messages[:1], manifest, True)
            with self.assertRaisesRegex(ValueError, 'duplicate'):
                candidate.record(root, REVISION, messages + messages[:1], manifest, True)
            candidate.record(root, REVISION, messages, manifest, True)
            body = json.loads(manifest.read_text()); body['identity']['features'] = 'default'
            manifest.write_text(json.dumps(body))
            with self.assertRaisesRegex(ValueError, 'fingerprint'):
                candidate.verify(root, REVISION, manifest, 'engine')
            outside = root / 'unexpected executable'; outside.write_bytes(b'exe')
            messages[0]['executable'] = str(outside)
            with self.assertRaisesRegex(ValueError, 'target directory'):
                candidate.record(root, REVISION, messages, manifest, True)

    def test_suites_do_not_repeat_full_self_documentation_checks(self):
        self.assertEqual(candidate.test_arguments('full'), [])
        self.assertEqual(candidate.test_arguments('sparql'), ['--test', 'sparql_endpoints'])
        self.assertIn('self_documentation', candidate.test_arguments('docs'))

    def test_rejects_wrong_executable_machine_even_with_an_authentic_hash(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(candidate, 'identity', return_value=IDENTITY):
            root, engine, _, messages, manifest = self.fixture(temporary)
            engine.write_bytes(b'\x7fELF\x02\x01' + b'\0' * 12 + struct.pack('<H', 183))
            with self.assertRaisesRegex(ValueError, 'machine'):
                candidate.record(root, REVISION, messages, manifest, True)

    def test_archive_corruption_and_other_producer_fingerprints_never_execute(self):
        with tempfile.TemporaryDirectory() as temporary, patch.object(candidate, 'identity', return_value=IDENTITY):
            root, _, _, messages, manifest = self.fixture(temporary)
            archive = root / 'candidate-tests.tar.zst'; archive.write_bytes(b'archive')
            body = candidate.record(root, REVISION, messages, manifest, True, archive)
            with patch.object(candidate.subprocess, 'run') as run:
                with self.assertRaisesRegex(ValueError, 'fingerprint'):
                    candidate.verify(root, REVISION, manifest, 'engine', 'b' * 64)
                archive.write_bytes(b'corrupt')
                # The archive resides beside its manifest in a real download.
                archive.replace(manifest.parent / archive.name)
                with self.assertRaisesRegex(ValueError, 'checksum'):
                    candidate.extract(root, REVISION, manifest, body['fingerprint'])
                run.assert_not_called()
