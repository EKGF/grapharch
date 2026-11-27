"""Re-run an automatic release attempt that failed only because no hosted runner was acquired.

GitHub-hosted runners have no configurable acquisition timeout: when no machine
is assigned for about 15 minutes, GitHub cancels the job before it starts and
records a failure annotation. Such a job never executed a step, so re-running it
is the same action as the manual "re-run failed jobs". Any job that started, or
any other failure, is a real result and is never retried.
"""
import argparse
import json
import os
import subprocess

ACQUISITION_FAILURE = 'The job was not acquired by Runner of type hosted even after multiple attempts'
WORKFLOW_PATH = '.github/workflows/automatic-release.yml'
# Attempts 2..MAX_ATTEMPTS are automatic; GitHub allows 50 re-runs per run.
MAX_ATTEMPTS = 4


def api(path):
    return json.loads(subprocess.check_output(['gh', 'api', path], text=True))


def attempt_jobs(repository, run_id, attempt):
    # bounded: one attempt of one release run.
    jobs = []
    for page in range(1, 11):
        records = api(f'repos/{repository}/actions/runs/{run_id}/attempts/{attempt}/jobs'
                      f'?per_page=100&page={page}')['jobs']
        jobs.extend(records)
        if len(records) < 100:
            return jobs
    raise ValueError('release attempt job list exceeds bounded review budget')


class Attempt:
    """One completed attempt of an automatic release run and its job outcomes."""

    def __init__(self, run, jobs, annotations):
        self.run = run
        self.jobs = jobs
        self.annotations = annotations

    def unacquired(self, job):
        """A job GitHub cancelled before any runner picked it up."""
        return (job.get('status') == 'completed'
                and job.get('conclusion') in ('cancelled', 'failure')
                and not job.get('runner_id') and not job.get('runner_name')
                and not job.get('steps')
                and any(note.get('annotation_level') == 'failure'
                        and note.get('message') == ACQUISITION_FAILURE
                        for note in self.annotations(job['id'])))

    def decision(self):
        """Return (retry, reason)."""
        run = self.run
        if run.get('path') != WORKFLOW_PATH or run.get('head_branch') != 'main':
            return False, 'not an automatic release run on main'
        if run.get('status') != 'completed' or run.get('conclusion') != 'failure':
            return False, f"run concluded {run.get('conclusion')}, not failure"
        if run.get('run_attempt', 0) >= MAX_ATTEMPTS:
            return False, f"attempt {run.get('run_attempt')} reached the automatic retry limit"
        unsuccessful = [job for job in self.jobs
                        if job.get('conclusion') not in ('success', 'skipped', 'neutral')]
        if not unsuccessful:
            return False, 'no unsuccessful job to retry'
        real = [job['name'] for job in unsuccessful if not self.unacquired(job)]
        if real:
            return False, 'real job failures: ' + ', '.join(sorted(real))
        return True, 'runner acquisition failed for: ' + ', '.join(
            sorted(job['name'] for job in unsuccessful))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', required=True, type=int)
    parser.add_argument('--attempt', required=True, type=int)
    args = parser.parse_args()
    repository = os.environ['GITHUB_REPOSITORY']
    run = api(f'repos/{repository}/actions/runs/{args.run}')
    if run.get('run_attempt') != args.attempt:
        # A newer attempt already exists (for example a manual re-run); it owns its result.
        print(f"Run {args.run} is at attempt {run.get('run_attempt')}, not {args.attempt}; nothing to do")
        return
    attempt = Attempt(run, attempt_jobs(repository, args.run, args.attempt),
                      lambda job: api(f'repos/{repository}/check-runs/{job}/annotations?per_page=100'))
    retry, reason = attempt.decision()
    print(reason)
    if retry:
        subprocess.run(['gh', 'run', 'rerun', str(args.run), '--failed', '--repo', repository], check=True)
        print(f'Re-ran failed jobs of run {args.run} as attempt {args.attempt + 1}')


if __name__ == '__main__':
    main()
