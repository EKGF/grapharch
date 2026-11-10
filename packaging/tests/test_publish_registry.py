"""Retries may skip only byte-identical immutable registry artifacts."""
import base64
import hashlib
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).parents[1]))
from registry_state import npm_published, pypi_published, crate_published


class RegistryState(unittest.TestCase):
    def test_crate_retry_requires_identical_source_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / 'grapharch.crate'
            archive.write_bytes(b'verified source')
            metadata = {'version': {'checksum': hashlib.sha256(archive.read_bytes()).hexdigest()}}
            with patch('registry_state.read_json', return_value=metadata):
                self.assertTrue(crate_published('0.2.3', archive))
            with patch('registry_state.read_json', return_value=None):
                self.assertFalse(crate_published('0.2.3', archive))
            metadata['version']['checksum'] = 'incorrect'
            with patch('registry_state.read_json', return_value=metadata):
                with self.assertRaisesRegex(ValueError, 'different bytes'):
                    crate_published('0.2.3', archive)

    def test_npm_retry_accepts_exact_integrity_only(self):
        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / 'package.tgz'
            archive.write_bytes(b'validated artifact')
            integrity = 'sha512-' + base64.b64encode(hashlib.sha512(archive.read_bytes()).digest()).decode()
            with patch('registry_state.read_json', return_value={'name':'grapharch','version':'0.2.3','dist': {'integrity': integrity}}):
                self.assertTrue(npm_published('grapharch', '0.2.3', archive))
            with patch('registry_state.read_json', return_value={'name':'grapharch','version':'0.2.3','dist': {'integrity': 'sha512-wrong'}}):
                with self.assertRaisesRegex(ValueError, 'different bytes'):
                    npm_published('grapharch', '0.2.3', archive)
            with patch('registry_state.read_json', return_value=None):
                self.assertFalse(npm_published('grapharch', '0.2.3', archive))

    def test_pypi_retry_does_not_hide_a_conflicting_wheel(self):
        with tempfile.TemporaryDirectory() as temporary:
            wheel = Path(temporary) / 'grapharch.whl'
            wheel.write_bytes(b'validated wheel')
            metadata = {'urls': [{'filename': wheel.name, 'digests': {'sha256': hashlib.sha256(wheel.read_bytes()).hexdigest()}}]}
            with patch('registry_state.read_json', return_value=metadata):
                self.assertTrue(pypi_published('0.2.3', wheel))
            metadata['urls'][0]['digests']['sha256'] = 'incorrect'
            with patch('registry_state.read_json', return_value=metadata):
                with self.assertRaisesRegex(ValueError, 'different bytes'):
                    pypi_published('0.2.3', wheel)

    def test_npm_registry_identity_is_exact_even_when_bytes_match(self):
        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / 'package.tgz'
            archive.write_bytes(b'verified artifact')
            integrity = 'sha512-' + base64.b64encode(hashlib.sha512(archive.read_bytes()).digest()).decode()
            for name, version in [('other-package', '0.2.7'), ('grapharch-linux-x64', '0.2.6')]:
                record = {'name': name, 'version': version, 'dist': {'integrity': integrity}}
                with patch('registry_state.read_json', return_value=record):
                    with self.assertRaises(ValueError):
                        npm_published('grapharch-linux-x64', '0.2.7', archive)
