from pathlib import Path
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).parents[1]))
from release_assets import verify_existing, verify_complete
from registry_state import digest


class ReleaseAssets(unittest.TestCase):
    def test_identical_existing_assets_are_idempotent_and_conflicts_fail(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'grapharch.zip'
            archive.write_bytes(b'verified executable archive')
            asset = {'name': archive.name, 'digest': 'sha256:' + digest(archive, 'sha256').hexdigest()}
            verify_existing([asset], root)
            asset['digest'] = 'sha256:incorrect'
            with self.assertRaisesRegex(ValueError, 'differs'):
                verify_existing([asset], root)
            with self.assertRaisesRegex(ValueError, 'unexpected'):
                verify_existing([{'name': 'unreviewed.zip'}], root)

    def test_artifact_only_handoff_requires_every_original_public_asset(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'grapharch.zip'
            archive.write_bytes(b'verified executable archive')
            asset = {'name': archive.name, 'digest': 'sha256:' + digest(archive, 'sha256').hexdigest()}
            with self.assertRaises(ValueError):
                verify_complete([], root)
            verify_complete([asset], root)
