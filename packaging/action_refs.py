"""Move documented Action convenience refs after complete publication."""
import argparse
import os
import re
import subprocess
import tomllib

from artifact_proof import jobs_for_run, latest_jobs
from distribute import PLATFORMS


def release_version(value):
    if not re.fullmatch(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)', value):
        raise ValueError('Action refs require a stable release version')
    return tuple(map(int, value.split('.')))


def may_advance(current, target):
    return release_version(current) <= release_version(target)


def require_complete_publication(jobs):
    latest = latest_jobs(jobs)
    required = {'distribute / ' + name for name in
                ('assemble', 'github-release', 'publish-pypi', 'publish-npm', 'publish-crate')}
    binaries = {name for name in latest if name.startswith('distribute / binaries (')}
    native_ready = (len(binaries) == len(PLATFORMS)
                    and all(latest[name].get('conclusion') == 'success' for name in binaries))
    reused = latest.get('distribute / verify-artifacts', {}).get('conclusion') == 'success'
    if not (native_ready or reused) or any(latest.get(name, {}).get('conclusion') != 'success'
                                         for name in required):
        raise ValueError('Action references require all six artifacts and every publication channel to succeed; skipped is not published')


def verify_publication_run():
    require_complete_publication(jobs_for_run(os.environ['GITHUB_REPOSITORY'], os.environ['GITHUB_RUN_ID']))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', required=True)
    args = parser.parse_args()
    verify_publication_run()
    major, minor, _ = release_version(args.version)
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
    tag = subprocess.check_output(['git', 'rev-parse', f'v{args.version}^{{commit}}'], text=True).strip()
    if head != tag:
        raise ValueError('Action upgrade must target its exact immutable release tag')
    refs = [f'refs/tags/v{major}', f'refs/tags/v{major}.{minor}']
    remote = subprocess.check_output(['git', 'ls-remote', '--refs', 'origin', *refs], text=True)
    previous = {line.split()[1]: line.split()[0] for line in remote.splitlines()}
    selected = []
    for ref in refs:
        if ref in previous:
            subprocess.run(['git', 'fetch', '--no-tags', 'origin', ref], check=True)
            manifest = subprocess.check_output(['git', 'show', 'FETCH_HEAD:Cargo.toml'], text=True)
            current = tomllib.loads(manifest)['package']['version']
            if not may_advance(current, args.version):
                print(f'Keep newer Action reference {ref} at version {current}')
                continue
        selected.append(ref)
    if not selected:
        return
    subprocess.run(['git', 'push', '--atomic',
                    *[f'--force-with-lease={ref}:{previous.get(ref, "")}' for ref in selected],
                    'origin', *[f'{head}:{ref}' for ref in selected]], check=True)


if __name__ == '__main__':
    main()
