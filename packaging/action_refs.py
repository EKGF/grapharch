"""Move documented Action convenience refs after complete publication."""
import argparse
import re
import subprocess
import tomllib


def release_version(value):
    if not re.fullmatch(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)', value):
        raise ValueError('Action refs require a stable release version')
    return tuple(map(int, value.split('.')))


def may_advance(current, target):
    return release_version(current) <= release_version(target)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', required=True)
    args = parser.parse_args()
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
