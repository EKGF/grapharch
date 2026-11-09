"""Release eligibility must not mistake docs-only validation for a candidate."""
import sys
from pathlib import Path
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))
from auto_release import eligible, next_version, REQUIRED_JOBS, verify_proof


class AutomaticRelease(unittest.TestCase):
    def run_record(self):
        return dict(event='push', head_branch='main', head_sha='a' * 40,
                    conclusion='success', path='.github/workflows/rust-ci.yml',
                    head_repository=dict(full_name='EKGF/grapharch'))

    def test_only_full_successful_main_push_is_eligible(self):
        record = self.run_record()
        jobs = [dict(name=name, conclusion='success') for name in REQUIRED_JOBS]
        self.assertTrue(eligible(record, jobs, 'EKGF/grapharch'))
        for key, value in [('event', 'workflow_dispatch'), ('conclusion', 'failure'),
                           ('head_branch', 'feature/test')]:
            altered = dict(record, **{key: value})
            self.assertFalse(eligible(altered, jobs, 'EKGF/grapharch'))
        self.assertFalse(eligible(record, jobs[:-1], 'EKGF/grapharch'))
        self.assertFalse(eligible(record, jobs, 'attacker/grapharch'))

    def test_patch_default_minor_explicit_and_major_forbidden(self):
        self.assertEqual(next_version('0.2.2', ['v0.2.2', 'v0.5.7']), '0.5.8')
        self.assertEqual(next_version('1.9.3', ['v1.10.0'], 'minor'), '1.11.0')
        with self.assertRaises(ValueError):
            next_version('0.2.2', ['v1.0.0'])
        with self.assertRaises(ValueError):
            next_version('0.2.2', [], 'major')

    def test_coordinator_is_serial_and_publisher_is_reused(self):
        root = Path(__file__).parents[2]
        workflow = (root / '.github/workflows/automatic-release.yml').read_text()
        self.assertIn('workflow_run:', workflow)
        self.assertIn('cancel-in-progress: false', workflow)
        self.assertIn('uses: ./.github/workflows/distribute.yml', workflow)
        self.assertIn('publish-packages: true', workflow)
        self.assertNotIn('cargo build', workflow)

    def test_source_checks_must_include_package_and_commit_validation(self):
        run = self.run_record()
        jobs = dict(jobs=[dict(name=name, conclusion='success') for name in REQUIRED_JOBS])
        checks = dict(workflow_runs=[dict(name=name, conclusion='success', id=index)
                                    for index, name in enumerate(('Check Commits', 'Package checks'))])
        with patch('auto_release.api', side_effect=[run, jobs, checks]):
            verify_proof('EKGF/grapharch', 42, 'a' * 40)
        checks['workflow_runs'][1]['conclusion'] = 'failure'
        with patch('auto_release.api', side_effect=[run, jobs, checks]):
            with self.assertRaisesRegex(ValueError, 'Package checks'):
                verify_proof('EKGF/grapharch', 42, 'a' * 40)

    def test_docs_only_success_cannot_reuse_full_source_proof(self):
        jobs = dict(jobs=[dict(name='scope', conclusion='success')])
        with patch('auto_release.api', side_effect=[self.run_record(), jobs]):
            with self.assertRaisesRegex(ValueError, 'full main-push'):
                verify_proof('EKGF/grapharch', 42, 'a' * 40)


if __name__ == '__main__':
    unittest.main()
