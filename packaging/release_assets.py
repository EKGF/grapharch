"""Never replace an existing immutable GitHub release asset with different bytes."""
import argparse
from pathlib import Path
import os
import subprocess
import json

from registry_state import digest


def verify_existing(assets, output):
    expected = {path.name: path for pattern in ('*.zip', '*.zip.sha256', '*.json', 'homebrew/grapharch.rb')
                for path in output.glob(pattern)}
    for asset in assets:
        if asset['name'] not in expected:
            raise ValueError(f'unexpected existing release asset: {asset["name"]}')
        checksum = 'sha256:' + digest(expected[asset['name']], 'sha256').hexdigest()
        if asset.get('digest') != checksum:
            raise ValueError(f'immutable release asset differs: {asset["name"]}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', required=True)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    # gh api preserves authentication and reports only public release metadata.
    result = subprocess.run(['gh', 'api', f'repos/{os.environ["GITHUB_REPOSITORY"]}/releases/tags/v{args.version}'],
                            capture_output=True, text=True)
    if result.returncode:
        if 'HTTP 404' not in result.stderr:
            raise ValueError('cannot inspect existing GitHub release: ' + result.stderr)
    else:
        verify_existing(json.loads(result.stdout)['assets'], args.output)
