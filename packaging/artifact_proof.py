"""Verify an assembled native distribution before a supported-event registry handoff."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess

from distribute import PLATFORMS


def api(path):
    return json.loads(subprocess.check_output(['gh', 'api', path], text=True))


def jobs_for_run(repository, run_id):
    # bounded: one release's job history, at most 1,000 records including retries.
    jobs = []
    for page in range(1, 11):
        records = api(f'repos/{repository}/actions/runs/{run_id}/jobs?filter=all&per_page=100&page={page}')['jobs']
        jobs.extend(records)
        if len(records) < 100:
            return jobs
    raise ValueError('release job history exceeds bounded review budget')


def latest_jobs(jobs):
    latest = {}
    for job in jobs:
        if job['id'] > latest.get(job['name'], {}).get('id', -1):
            latest[job['name']] = job
    return latest


def validate_run(run, jobs, repository, source):
    if (run['path'] != '.github/workflows/automatic-release.yml'
            or run['event'] not in ('workflow_run', 'workflow_dispatch')
            or run['head_branch'] != 'main' or run['head_sha'] != source
            or run['head_repository']['full_name'].lower() != repository.lower()):
        raise ValueError('artifact producer must be the same trusted main release source')
    latest = latest_jobs(jobs)
    required = {'reserve', 'distribute / metadata', 'distribute / verify-proof',
                'distribute / assemble', 'distribute / github-release'}
    targets = {item['target'] for item in PLATFORMS}
    for target in targets:
        matches = [name for name in latest if name.startswith(f'distribute / binaries ({target},')]
        if len(matches) != 1:
            raise ValueError(f'missing or ambiguous native producer for {target}')
        required.add(matches[0])
    if any(latest.get(name, {}).get('conclusion') != 'success' for name in required):
        raise ValueError('artifact reuse requires every native build, proof, assembly and GitHub publication')


def validate_directory(directory, revision, version):
    if re.fullmatch('[a-f0-9]{40}', revision) is None or revision == '0' * 40:
        raise ValueError('artifact source must be an exact nonzero revision')
    for item in PLATFORMS:
        path = directory / f'grapharch-{item["target"]}.json'
        if path.stat().st_size > 64 * 1024:
            raise ValueError('native manifest exceeds bounded metadata budget')
        manifest = json.loads(path.read_text())
        if (manifest.get('source_commit') != revision or manifest.get('version') != version
                or manifest.get('target') != item['target']):
            raise ValueError('assembled artifact differs from the requested release source/version/target')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', default=os.environ.get('GITHUB_REPOSITORY'))
    parser.add_argument('--run', type=int)
    parser.add_argument('--source')
    parser.add_argument('--verified-run', type=int)
    parser.add_argument('--directory', type=Path)
    parser.add_argument('--revision')
    parser.add_argument('--version')
    args = parser.parse_args()
    if args.directory is not None:
        if not args.revision or not args.version:
            raise ValueError('artifact validation requires release revision and version')
        validate_directory(args.directory, args.revision, args.version)
    else:
        if not args.run or args.run < 1 or not args.source or not args.verified_run or args.verified_run < 1:
            raise ValueError('artifact reuse requires a producer run and verified source')
        validate_run(api(f'repos/{args.repository}/actions/runs/{args.run}'),
                     jobs_for_run(args.repository, args.run), args.repository, args.source)
        artifacts = api(f'repos/{args.repository}/actions/runs/{args.run}/artifacts?per_page=100')['artifacts']
        selected = [artifact for artifact in artifacts if artifact['name'] == 'release-distribution']
        if len(selected) != 1 or selected[0]['expired']:
            raise ValueError('one retained assembled release artifact is required')


if __name__ == '__main__':
    main()
