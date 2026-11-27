"""Only release attempts that failed for want of a hosted runner are re-run, a bounded number of times."""
import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))
from runner_retry import ACQUISITION_FAILURE, MAX_ATTEMPTS, Attempt

ROOT = Path(__file__).resolve().parents[2]
ARM64 = 'distribute / binaries (aarch64-apple-darwin, darwin-arm64, darwin, arm64, macos-15, off)'


def run(attempt=1, conclusion='failure'):
    return {'path': '.github/workflows/automatic-release.yml', 'head_branch': 'main',
            'status': 'completed', 'conclusion': conclusion, 'run_attempt': attempt}


def unacquired(job_id=134902875235):
    # Shape of job 134902875235, run 44333024358 attempt 1 (v0.2.30).
    return {'id': job_id, 'name': ARM64, 'status': 'completed', 'conclusion': 'cancelled',
            'runner_id': 0, 'runner_name': '', 'steps': []}


def jobs(*extra):
    return [{'id': 1, 'name': 'reserve', 'status': 'completed', 'conclusion': 'success',
             'runner_id': 7, 'runner_name': 'GitHub Actions 7', 'steps': [{}]},
            unacquired(),
            {'id': 3, 'name': 'distribute / assemble', 'status': 'completed', 'conclusion': 'skipped',
             'runner_id': 0, 'runner_name': '', 'steps': []}, *extra]


ACQUISITION_NOTES = [{'annotation_level': 'failure', 'message': ACQUISITION_FAILURE},
                     {'annotation_level': 'notice', 'message': 'Due to capacity constraints, jobs targeting '
                      'macOS arm64 runners may experience longer queue times.'}]


def notes(mapping):
    return lambda job: mapping.get(job, [])


class RunnerRetry(unittest.TestCase):
    def test_unacquired_macos_runner_is_retried(self):
        retry, reason = Attempt(run(), jobs(), notes({134902875235: ACQUISITION_NOTES})).decision()
        self.assertTrue(retry)
        self.assertIn(ARM64, reason)

    def test_retries_are_bounded(self):
        for attempt in range(1, MAX_ATTEMPTS):
            self.assertTrue(Attempt(run(attempt), jobs(), notes({134902875235: ACQUISITION_NOTES})).decision()[0])
        retry, reason = Attempt(run(MAX_ATTEMPTS), jobs(), notes({134902875235: ACQUISITION_NOTES})).decision()
        self.assertFalse(retry)
        self.assertIn('limit', reason)

    def test_real_build_failure_is_never_retried(self):
        failed = {'id': 4, 'name': 'distribute / binaries (x86_64-unknown-linux-gnu, ...)', 'status': 'completed',
                  'conclusion': 'failure', 'runner_id': 9, 'runner_name': 'GitHub Actions 9', 'steps': [{}]}
        retry, reason = Attempt(run(), jobs(failed), notes({134902875235: ACQUISITION_NOTES})).decision()
        self.assertFalse(retry)
        self.assertIn('x86_64-unknown-linux-gnu', reason)

    def test_acquisition_is_proved_by_github_annotation_not_by_job_shape(self):
        # An unstarted cancelled job without GitHub's annotation (e.g. a manual cancel) is not retried.
        self.assertFalse(Attempt(run(), jobs(), notes({})).decision()[0])
        notice_only = {134902875235: ACQUISITION_NOTES[1:]}
        self.assertFalse(Attempt(run(), jobs(), notes(notice_only)).decision()[0])

    def test_a_job_that_started_is_not_an_acquisition_failure(self):
        started = dict(unacquired(), runner_id=5, runner_name='GitHub Actions 5', steps=[{}])
        self.assertFalse(Attempt(run(), [started], notes({134902875235: ACQUISITION_NOTES})).decision()[0])

    def test_cancelled_or_successful_runs_and_other_workflows_are_ignored(self):
        lookup = notes({134902875235: ACQUISITION_NOTES})
        for conclusion in ('success', 'cancelled'):
            self.assertFalse(Attempt(run(conclusion=conclusion), jobs(), lookup).decision()[0])
        other = dict(run(), path='.github/workflows/distribute.yml')
        self.assertFalse(Attempt(other, jobs(), lookup).decision()[0])
        branch = dict(run(), head_branch='feature')
        self.assertFalse(Attempt(branch, jobs(), lookup).decision()[0])

    def test_retry_workflow_is_a_bounded_trusted_follow_up_of_the_release(self):
        workflow = (ROOT / '.github/workflows/release-runner-retry.yml').read_text(encoding='utf-8')
        self.assertIn("workflows: ['Automatic release']", workflow)
        self.assertIn('types: [completed]', workflow)
        self.assertIn(f'github.event.workflow_run.run_attempt < {MAX_ATTEMPTS}', workflow)
        self.assertIn("github.event.workflow_run.conclusion == 'failure'", workflow)
        self.assertIn('ref: main', workflow)
        self.assertIn('packaging/runner_retry.py --run "$RELEASE_RUN" --attempt "$RELEASE_ATTEMPT"', workflow)
        self.assertRegex(workflow, r'(?m)^permissions:\n  contents: read\n')
        self.assertRegex(workflow, r'      actions: write\n      checks: read\n')
        self.assertNotIn('pull_request', workflow)
        self.assertNotRegex(workflow, r'set -e|contents: write|id-token')
        release = (ROOT / '.github/workflows/automatic-release.yml').read_text(encoding='utf-8')
        self.assertIn('name: Automatic release\n', release)
        # The released run itself does not re-run anything; GitHub cannot re-run an in-progress run.
        self.assertNotIn('gh run rerun', release)
        script = (ROOT / 'packaging/runner_retry.py').read_text(encoding='utf-8')
        self.assertEqual(len(re.findall(r"'gh', 'run', 'rerun'", script)), 1)
        self.assertIn("'--failed'", script)


if __name__ == '__main__':
    unittest.main()
