"""Select candidate checks without starting independent compiler workflows."""
import json
import os
from pathlib import Path
import re
import subprocess


def select(mode, event, ref, paths):
    if mode not in ('candidate', 'docs', 'sparql'):
        raise ValueError('unknown candidate scope')
    # New branches and explicit candidate dispatches deliberately verify all OSes.
    full = mode == 'candidate' and (ref == 'refs/heads/main' or paths is None or any(
        not p.startswith('docs/') or p.startswith('docs/other/ontology/')
        for p in paths))
    relevant = paths is None or any(
        p.startswith(('docs/', 'examples/', 'src/', 'tests/', '.github/'))
        or p in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'build.rs')
        for p in paths)
    docs = mode == 'docs' or (mode == 'candidate' and relevant
        and (event != 'push' or ref == 'refs/heads/main'))
    return {'desktop': full, 'docs': docs, 'sparql': mode != 'docs',
            'suite': 'full' if full else ('sparql' if mode == 'sparql' else 'docs')}


def changed(root, event, payload):
    if event == 'push':
        base = payload['before']
        if base == '0' * 40:
            return None
    elif event == 'pull_request':
        base = payload['pull_request']['base']['sha']
    else:
        return None
    if not re.fullmatch('[a-f0-9]{40}', base):
        raise ValueError('event base must be an exact Git revision')
    # A force-pushed prior revision can be absent even in a full clone.
    exists = subprocess.run(['git', 'cat-file', '-e', base + '^{commit}'], cwd=root, capture_output=True)
    if exists.returncode:
        subprocess.run(['git', 'fetch', '--no-tags', 'origin', base], cwd=root, check=True)
    command = ['git', 'diff', '--name-only', '-z', base, 'HEAD']
    with subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE) as process:
        paths = process.stdout.read(4 * 1024 * 1024 + 1)
        if len(paths) > 4 * 1024 * 1024:
            process.kill()
            process.wait()
            raise ValueError('candidate changed-path report exceeds budget')
        if process.wait() != 0:
            raise ValueError('cannot resolve candidate changed paths')
    return [p.decode('utf-8') for p in paths.split(b'\0') if p]


def main():
    root = Path(__file__).resolve().parents[1]
    head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
    if head != os.environ['GITHUB_SHA']:
        raise ValueError('candidate checkout differs from event revision')
    event = os.environ['GITHUB_EVENT_NAME']
    payload = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text())
    result = select(os.environ.get('CANDIDATE_SCOPE', 'candidate'), event,
                    os.environ['GITHUB_REF'], changed(root, event, payload))
    result['revision'] = head
    with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
        for key, value in result.items():
            output.write(f'{key}={str(value).lower() if isinstance(value, bool) else value}\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
