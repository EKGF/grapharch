"""Reserve one patch or explicitly selected minor release from a fully verified, current main push."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import tomllib

from signed_commit import create as create_signed_commit

REQUIRED_JOBS = {
    'scope', 'Compile the Linux candidate once', 'Linux source tests',
    'Real SPARQL NativeStore', 'Verify showcases and publish documentation',
    'Format, Lint, Test (windows-latest)', 'Format, Lint, Test (macos-latest)',
    'pages-deploy',
}
FILES = ('Cargo.toml', 'Cargo.lock', 'npm/package.json',
         'docs/other/ontology/publication.ttl')


def git(*args):
    return subprocess.check_output(['git', *args], text=True).strip()


def api(path):
    return json.loads(subprocess.check_output(['gh', 'api', path], text=True))


def eligible(run, jobs, repository):
    return (run['event'] == 'push' and run['head_branch'] == 'main'
            and run['conclusion'] == 'success'
            and run['path'] == '.github/workflows/rust-ci.yml'
            and run['head_repository']['full_name'].lower() == repository.lower()
            and re.fullmatch('[a-f0-9]{40}', run['head_sha']) is not None
            and REQUIRED_JOBS <= {j['name'] for j in jobs if j['conclusion'] == 'success'})


def next_version(current, tags, bump="patch"):
    source = tuple(map(int, current.split('.')))
    published = []
    for tag in tags:
        if re.fullmatch(r'v\d+\.\d+\.\d+', tag):
            published.append(tuple(map(int, tag[1:].split('.'))))
    if published and max(published)[0] != source[0]:
        raise ValueError('published major differs from source; explicit review required')
    if not published and source[0] != 0:
        raise ValueError('initial major release requires explicit review')
    versions = [source, *published]
    major, minor, patch = max(versions)
    if major != versions[0][0]:
        raise ValueError('published major differs from source; explicit review required')
    if bump == 'patch':
        return f'{major}.{minor}.{patch + 1}'
    if bump == 'minor':
        return f'{major}.{minor + 1}.0'
    raise ValueError('automatic major releases are forbidden')


def select_bump(message, override):
    signals = [line.removeprefix('GraphArch-Release-Bump: ').strip()
               for line in message.splitlines() if line.startswith('GraphArch-Release-Bump:')]
    if len(signals) > 1 or any(signal not in ('patch', 'minor') for signal in signals):
        raise ValueError('release signal must be a single patch or minor trailer; major requires separate approval')
    if override not in ('auto', 'patch', 'minor'):
        raise ValueError('unsupported release override')
    return override if override != 'auto' else (signals[0] if signals else 'patch')


def verify_proof(repository, run_id, source):
    run = api(f'repos/{repository}/actions/runs/{run_id}')
    jobs = api(f'repos/{repository}/actions/runs/{run_id}/jobs?per_page=100')['jobs']
    if not eligible(run, jobs, repository) or run['head_sha'] != source:
        raise ValueError('release requires the exact successful full main-push candidate')
    compiler = next(job for job in jobs if job['name'] == 'Compile the Linux candidate once')
    if not any(step['name'] == 'Verify portable release dependency graphs' and step['conclusion'] == 'success'
               for step in compiler.get('steps', [])):
        raise ValueError('release proof must include the portable release dependency guard')
    checks = api(f'repos/{repository}/actions/runs?head_sha={source}&event=push&per_page=100')['workflow_runs']
    for name in ('Check Commits', 'Package checks'):
        matching = [item for item in checks if item['name'] == name]
        if not matching or max(matching, key=lambda item: item['id'])['conclusion'] != 'success':
            raise ValueError(f'release requires successful {name} for the same source')


def prepare(repository, run_id, output, bump):
    run = api(f'repos/{repository}/actions/runs/{run_id}')
    source = run['head_sha']
    verify_proof(repository, run_id, source)
    subprocess.run(['git', 'fetch', 'origin', 'main', '--tags'], check=True)
    bump = select_bump(git('show', '-s', '--format=%B', source), bump)
    main = git('rev-parse', 'origin/main')
    message = git('show', '-s', '--format=%B', main)
    marker = f'GraphArch-Release-Source: {source}'
    reservations = []
    tags = git('tag', '--list', 'v*').splitlines()
    if len(tags) > 4096:
        raise ValueError('release tag inventory exceeds bounded review budget')
    for tag in tags:
        if re.fullmatch(r'v\d+\.\d+\.\d+', tag):
            revision = git('rev-parse', f'{tag}^{{commit}}')
            if marker in git('show', '-s', '--format=%B', revision).splitlines():
                reservations.append(revision)
    if len(reservations) > 1:
        raise ValueError('source has conflicting release reservations')
    if reservations:
        main = reservations[0]
        message = git('show', '-s', '--format=%B', main)
    if marker in message.splitlines():
        # A retry resumes the same immutable reservation, never bumps again.
        subprocess.run(['git', 'checkout', '--detach', main], check=True)
        version = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
        if git('rev-parse', f'v{version}^{{commit}}') != main:
            raise ValueError('release reservation tag differs from its commit')
        revision = main
    elif main != source:
        print('Superseded candidate: main advanced; no release reserved')
        return
    else:
        subprocess.run(['git', 'checkout', '--detach', source], check=True)
        current = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
        version = next_version(current, git('tag', '--list', 'v*').splitlines(), bump)
        manifest = Path('Cargo.toml')
        manifest.write_text(manifest.read_text().replace(
            f'version = "{current}"', f'version = "{version}"', 1))
        lock = Path('Cargo.lock')
        old = f'name = "grapharch"\nversion = "{current}"'
        if lock.read_text().count(old) != 1:
            raise ValueError('lockfile must identify exactly one source package')
        lock.write_text(lock.read_text().replace(old, f'name = "grapharch"\nversion = "{version}"'))
        subprocess.run(['python3', '-B', 'packaging/distribute.py', 'sync-version'], check=True)
        changed = git('diff', '--name-only').splitlines()
        if set(changed) != set(FILES):
            raise ValueError('release preparation changed files beyond version metadata')
        revision = create_signed_commit(repository, source, FILES,
                                        f'chore(release): publish {version} [skip ci]',
                                        f'{marker}\nGraphArch-Release-Run: {run_id}')
        subprocess.run(['git', 'tag', '-a', f'v{version}', '-m', f'GraphArch {version}; verified source {source}'], check=True)
        # GITHUB_TOKEN pushes deliberately do not trigger another push workflow.
        # Atomic compare-and-swap rejects a racing main push without rewriting it.
        subprocess.run(['git', 'push', '--atomic', 'origin', 'HEAD:refs/heads/main',
                        f'refs/tags/v{version}'], check=True)
    with output.open('a') as stream:
        stream.write(f'revision={revision}\nsource={source}\nrun={run_id}\nversion={version}\nready=true\n')


def verify_release(repository, run_id, revision, source):
    verify_proof(repository, run_id, source)
    if git('rev-parse', 'HEAD') != revision or git('rev-parse', 'HEAD^') != source:
        raise ValueError('release checkout must be the verified source plus one metadata commit')
    message = git('show', '-s', '--format=%B')
    if f'GraphArch-Release-Source: {source}' not in message.splitlines():
        raise ValueError('release source trailer missing')
    changed = set(git('diff', '--name-only', source, revision).splitlines())
    if changed != set(FILES):
        raise ValueError('release differs from verified code beyond version metadata')
    old = tomllib.loads(git('show', f'{source}:Cargo.toml'))['package']['version']
    new = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
    if new.split('.')[0] != old.split('.')[0] or tuple(map(int, new.split('.'))) <= tuple(map(int, old.split('.'))):
        raise ValueError('release cannot automatically change major or decrease version')
    # Compare metadata semantics too: arbitrary edits inside allowed files are forbidden.
    for name in FILES:
        before = git('show', f'{source}:{name}')
        after = Path(name).read_text().strip()
        old = tomllib.loads(git('show', f'{source}:Cargo.toml'))['package']['version']
        new = tomllib.loads(Path('Cargo.toml').read_text())['package']['version']
        if name == 'npm/package.json':
            a, b = json.loads(before), json.loads(after)
            a['version'] = new
            a['optionalDependencies'] = {key: new for key in a['optionalDependencies']}
            valid = a == b
        elif name == 'Cargo.lock':
            valid = before.replace(f'name = "grapharch"\nversion = "{old}"',
                                   f'name = "grapharch"\nversion = "{new}"') == after
        elif name == 'Cargo.toml':
            valid = before.replace(f'version = "{old}"', f'version = "{new}"', 1) == after
        else:
            valid = before.replace(f'grapharch/{old}>', f'grapharch/{new}>').replace(
                f'owl:versionInfo "{old}', f'owl:versionInfo "{new}') == after
        if not valid:
            raise ValueError(f'release changed non-version content: {name}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('prepare', 'verify'))
    parser.add_argument('--run', required=True, type=int)
    parser.add_argument('--repository', default=os.environ.get('GITHUB_REPOSITORY'))
    parser.add_argument('--revision')
    parser.add_argument('--source')
    parser.add_argument('--bump', choices=('auto', 'patch', 'minor'), default='auto')
    args = parser.parse_args()
    if args.command == 'prepare':
        prepare(args.repository, args.run, Path(os.environ['GITHUB_OUTPUT']), args.bump)
    else:
        verify_release(args.repository, args.run, args.revision, args.source)


if __name__ == '__main__':
    main()
