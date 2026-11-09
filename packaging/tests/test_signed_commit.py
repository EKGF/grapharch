"""Never publish an unsigned or altered API-generated version commit."""
import sys
from pathlib import Path
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))
from signed_commit import validate_commit


class SignedCommit(unittest.TestCase):
    def record(self):
        return {'sha': 'b' * 40, 'parents': [{'sha': 'a' * 40}],
                'commit': {'tree': {'sha': 'c' * 40}, 'verification': {'verified': True}}}

    def test_requires_verified_signature_exact_parent_and_prepared_tree(self):
        record = self.record()
        self.assertEqual(validate_commit(record, 'a' * 40, 'c' * 40), 'b' * 40)
        for altered in [dict(record, parents=[]), dict(record, parents=[{'sha': 'd' * 40}])]:
            with self.assertRaises(ValueError):
                validate_commit(altered, 'a' * 40, 'c' * 40)
        record['commit']['verification']['verified'] = False
        with self.assertRaises(ValueError):
            validate_commit(record, 'a' * 40, 'c' * 40)
        record = self.record()
        record['commit']['tree']['sha'] = 'd' * 40
        with self.assertRaises(ValueError):
            validate_commit(record, 'a' * 40, 'c' * 40)

    def test_api_failure_never_falls_back_to_unsigned_commit(self):
        from signed_commit import create
        with patch('signed_commit.git', return_value='c' * 40), patch('signed_commit.subprocess.run') as command, patch('signed_commit.api', side_effect=[{}, RuntimeError('API unavailable')]), patch('signed_commit.Path.read_bytes', return_value=b'version'):
            with self.assertRaises(RuntimeError):
                create('EKGF/grapharch', 'a' * 40, ('Cargo.toml',), 'release', 'proof')
            self.assertFalse(any('commit' in call.args[0] for call in command.call_args_list))
            self.assertEqual(command.call_args_list[-1].args[0][:4], ['gh', 'api', '--method', 'DELETE'])
