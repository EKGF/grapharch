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
subprocess.run(['git', 'push', '--atomic', 'origin',
                f'{head}:refs/tags/v{major}', f'{head}:refs/tags/v{major}.{minor}',
                '--force'], check=True)
