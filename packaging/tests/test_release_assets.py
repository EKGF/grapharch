from pathlib import Path
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).parents[1]))
from release_assets import verify_existing
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
