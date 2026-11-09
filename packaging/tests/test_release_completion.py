"""A successful outer workflow is insufficient when publication jobs were skipped."""
import sys
import re
from pathlib import Path
import unittest
sys.path.insert(0, str(Path(__file__).parents[1]))
from action_refs import require_complete_publication


class Completion(unittest.TestCase):
    def records(self):
        names = ['assemble', 'github-release', 'publish-pypi', 'publish-npm', 'publish-crate']
        return [{'id': index, 'name': 'distribute / ' + name, 'conclusion': 'success'}
                for index, name in enumerate(names)] + [
                    {'id': 10 + index, 'name': f'distribute / binaries (target{index})', 'conclusion': 'success'}
                    for index in range(6)]

    def test_every_real_publication_stage_must_succeed_before_action_upgrade(self):
        require_complete_publication(self.records())
        for state in ['skipped', 'failure', None]:
            records = self.records()
            records[0]['conclusion'] = state
            with self.assertRaises(ValueError):
                require_complete_publication(records)
        with self.assertRaises(ValueError):
            require_complete_publication(self.records()[:-1])

    def test_retry_uses_latest_result_for_each_stage(self):
        records = self.records()
        records.append({'id': 100, 'name': 'distribute / publish-npm', 'conclusion': 'failure'})
        with self.assertRaises(ValueError):
            require_complete_publication(records)
        records.append({'id': 101, 'name': 'distribute / publish-npm', 'conclusion': 'success'})
        require_complete_publication(records)

    def test_intentionally_skipped_source_retest_does_not_skip_publication_descendants(self):
        root = Path(__file__).parents[2]
        workflow = (root / '.github/workflows/distribute.yml').read_text()
        for job, dependency in [('assemble', 'binaries'), ('github-release', 'assemble'),
                                ('publish-pypi', 'github-release'), ('publish-npm', 'github-release'),
                                ('publish-crate', 'github-release')]:
            section = re.split(r'\n  [a-z]', workflow.split('  ' + job + ':\n', 1)[1], maxsplit=1)[0]
            self.assertIn('!cancelled()', section)
            self.assertIn('needs.' + dependency + ".result == 'success'", section)
        refs = (root / '.github/workflows/automatic-release.yml').read_text().split('  action-refs:', 1)[1]
        self.assertIn('actions: read', refs)
        self.assertIn('GH_TOKEN:', refs)

    def test_verified_artifact_reuse_requires_all_real_publication_stages(self):
        records = [job for job in self.records() if not job['name'].startswith('distribute / binaries (')]
        with self.assertRaises(ValueError):
            require_complete_publication(records)
        records.append({'id': 100, 'name': 'distribute / verify-artifacts', 'conclusion': 'success'})
        require_complete_publication(records)
        records[0]['conclusion'] = 'skipped'
        with self.assertRaises(ValueError):
            require_complete_publication(records)
