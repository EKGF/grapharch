"""Check immutable registry bytes before safely resuming a partial publication."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import urllib.error
import urllib.parse
import urllib.request


def read_json(url):
    request = urllib.request.Request(url, headers={'User-Agent': 'GraphArch release verification (https://github.com/EKGF/grapharch)'})
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            data = response.read(4 * 1024 * 1024 + 1)
    except urllib.error.HTTPError as error:
        if error.code == 404:
            return None
        raise
    if len(data) > 4 * 1024 * 1024:
        raise ValueError('registry metadata exceeds bounded verification budget')
    return json.loads(data)


def digest(path, algorithm):
    result = hashlib.new(algorithm)
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(chunk)
    return result


def npm_published(name, version, archive):
    metadata = read_json(f'https://registry.npmjs.org/{urllib.parse.quote(name, safe="")}/{version}')
    if metadata is None:
        return False
    expected = 'sha512-' + base64.b64encode(digest(archive, 'sha512').digest()).decode()
    if metadata['dist']['integrity'] != expected:
        raise ValueError(f'npm {name}@{version} already exists with different bytes')
    return True


def pypi_published(version, wheel):
    metadata = read_json(f'https://pypi.org/pypi/grapharch/{version}/json')
    if metadata is None:
        return False
    entries = [item for item in metadata['urls'] if item['filename'] == wheel.name]
    if not entries:
        return False
    if len(entries) != 1 or entries[0]['digests']['sha256'] != digest(wheel, 'sha256').hexdigest():
        raise ValueError(f'PyPI {wheel.name} already exists with different bytes')
    return True


def crate_published(version, archive):
    metadata = read_json(f'https://crates.io/api/v1/crates/grapharch/{version}')
    if metadata is None:
        return False
    if metadata['version']['checksum'] != digest(archive, 'sha256').hexdigest():
        raise ValueError(f'crates.io grapharch {version} already exists with different bytes')
    return True


if __name__ == '__main__':
    from distribute import verify_release, version
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    verify_release(args.output)
    for wheel in args.output.glob('*.whl'):
        pypi_published(version(), wheel)
