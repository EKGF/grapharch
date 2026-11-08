"""Install one exact, reviewed GraphArch source checkout on Linux runners."""
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tomllib


def validate_selection(version, revision):
    if bool(version) == bool(revision):
        raise ValueError('supply exactly one of version or source-revision')
    if version:
        if not re.fullmatch(r'\d+\.\d+\.\d+', version):
            raise ValueError('version must be an exact MAJOR.MINOR.PATCH release without v')
        return 'release'
    if not re.fullmatch(r'[a-f0-9]{40}', revision) or revision == '0' * 40:
        raise ValueError('source-revision must be an exact nonzero lowercase Git SHA')
    return 'source'


def validate_checkout(root, revision):
    validate_selection('', revision)
    actual = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
    if actual != revision:
        raise ValueError('action checkout does not match source-revision')
    dirty = subprocess.check_output(['git', '-C', str(root), 'status', '--porcelain',
                                     '--untracked-files=all'], text=True)
    ignored = subprocess.check_output(['git', '-C', str(root), 'ls-files', '--others',
                                       '--ignored', '--exclude-standard', '--',
                                       'src', 'packaging', '.cargo'], text=True)
    if dirty or ignored:
        raise ValueError('source action requires a clean checkout without extra executable inputs')


def install(root, revision):
    validate_checkout(root, revision)
    if platform.system() != 'Linux':
        raise ValueError('source-revision installation currently supports Linux runners only')
    toolchain = tomllib.loads((root / 'rust-toolchain.toml').read_text())['toolchain']['channel']
    if not re.fullmatch(r'(nightly-\d{4}-\d{2}-\d{2}|\d+\.\d+\.\d+)', toolchain):
        raise ValueError('source action requires an exact pinned Rust toolchain')
    subprocess.run([sys.executable, str(root / 'packaging/install_build_dependencies.py')], check=True)
    subprocess.run(['rustup', 'toolchain', 'install', toolchain, '--profile', 'minimal'], check=True)
    directory = Path(os.environ['RUNNER_TEMP']) / 'grapharch-source' / revision
    subprocess.run(['cargo', '+' + toolchain, 'build', '--locked', '--release', '--bin', 'grapharch',
                    '--target-dir', str(directory)], cwd=root, check=True, env={**os.environ, 'GRAPHARCH_BUILD_REVISION': revision, 'GRAPHARCH_BUILD_RELEASE_VERSION': ''})
    binary = directory / 'release/grapharch'
    version = tomllib.loads((root / 'Cargo.toml').read_text())['package']['version']
    actual = subprocess.check_output([str(binary), '--version'], text=True).strip()
    if actual != f'grapharch {version}-dev+{revision[:12]}':
        raise ValueError('source binary version differs from pinned manifest')
    with open(os.environ['GITHUB_PATH'], 'a') as output:
        output.write(str(binary.parent) + '\n')
    with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
        output.write(f'version={version}\nrevision={revision}\nbinary-path={binary}\n')


def main():
    version = os.environ.get('GRAPHARCH_VERSION', '')
    revision = os.environ.get('GRAPHARCH_SOURCE_REVISION', '')
    mode = validate_selection(version, revision)
    if sys.argv[1] == 'validate':
        if mode == 'source':
            validate_checkout(Path(os.environ['GITHUB_ACTION_PATH']), revision)
    elif sys.argv[1] == 'install' and mode == 'source':
        install(Path(os.environ['GITHUB_ACTION_PATH']), revision)
    else:
        raise ValueError('expected validate, or install with source-revision')


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f'GraphArch source setup: {error}', file=sys.stderr)
        sys.exit(1)
