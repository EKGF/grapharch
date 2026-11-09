"""Move documented Action convenience refs after complete publication."""
import argparse
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--version', required=True)
args = parser.parse_args()
match = re.fullmatch(r'(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)', args.version)
if not match:
    raise ValueError('Action refs require a stable release version')
major, minor, _ = match.groups()
head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip()
tag = subprocess.check_output(['git', 'rev-parse', f'v{args.version}^{{commit}}'], text=True).strip()
if head != tag:
    raise ValueError('Action upgrade must target its exact immutable release tag')
refs = [f'refs/tags/v{major}', f'refs/tags/v{major}.{minor}']
remote = subprocess.check_output(['git', 'ls-remote', '--refs', 'origin', *refs], text=True)
previous = {line.split()[1]: line.split()[0] for line in remote.splitlines()}
subprocess.run(['git', 'push', '--atomic',
                *[f'--force-with-lease={ref}:{previous.get(ref, "")}' for ref in refs],
                'origin', *[f'{head}:{ref}' for ref in refs]], check=True)
