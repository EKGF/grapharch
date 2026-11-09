"""Never replace an existing immutable GitHub release asset with different bytes."""
import argparse
from pathlib import Path
import os
import subprocess
import json

from registry_state import digest


def expected_assets(output):
    return {path.name: path for pattern in ('*.zip', '*.zip.sha256', '*.json', 'homebrew/grapharch.rb')
                for path in output.glob(pattern)}


def verify_existing(assets, output):
    expected = expected_assets(output)
    for asset in assets:
        if asset['name'] not in expected:
            raise ValueError(f'unexpected existing release asset: {asset["name"]}')
        checksum = 'sha256:' + digest(expected[asset['name']], 'sha256').hexdigest()
        if asset.get('digest') != checksum:
            raise ValueError(f'immutable release asset differs: {asset["name"]}')


def verify_complete(assets, output):
    if {asset['name'] for asset in assets} != set(expected_assets(output)):
        raise ValueError('artifact-only publication requires every original GitHub asset')
    verify_existing(assets, output)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', required=True)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--require-complete', action='store_true')
    args = parser.parse_args()
    # gh api preserves authentication and reports only public release metadata.
    result = subprocess.run(['gh', 'api', f'repos/{os.environ["GITHUB_REPOSITORY"]}/releases/tags/v{args.version}'],
                            capture_output=True, text=True)
    if result.returncode:
        if args.require_complete or 'HTTP 404' not in result.stderr:
            raise ValueError('cannot inspect existing GitHub release: ' + result.stderr)
    else:
        check = verify_complete if args.require_complete else verify_existing
        check(json.loads(result.stdout)['assets'], args.output)
