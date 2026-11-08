"""Build once, then execute only the exact candidate engine/test harness."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tomllib

from source_action import validate_checkout
from distribute import platform as release_platform, verify_machine

DOC_TESTS = ('showcase_gallery', 'taxonomy_use_case_showcases', 'specification_sidebar',
             'dprod_showcase', 'w3c_specification', 'self_documentation', 'ontology_specification',
             'publication_ontology', 'authority_governance', 'ontology_family', 'repository_sources')
ENVIRONMENT = ('GRAPHARCH_BUILD_REVISION', 'GRAPHARCH_BUILD_RELEASE_VERSION', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET', 'CARGO_INCREMENTAL',
               'CARGO_PROFILE_DEV_DEBUG', 'CARGO_PROFILE_TEST_DEBUG', 'RUSTC_WRAPPER', 'CARGO_TARGET_DIR', 'CARGO_BUILD_BUILD_DIR', 'RUSTC', 'RUSTC_WORKSPACE_WRAPPER')
MAX_BINARY = 512 * 1024 * 1024


def digest(path):
    with path.open('rb') as source:
        return hashlib.file_digest(source, 'sha256').hexdigest()


def identity(root, revision):
    validate_checkout(root, revision)
    if os.environ.get('GRAPHARCH_BUILD_REVISION') not in (None, '', revision):
        raise ValueError('compiled generator identity differs from candidate revision')
    compiler = subprocess.check_output(['rustc', '--version', '--verbose'], cwd=root, text=True)
    hosts = [line[6:] for line in compiler.splitlines() if line.startswith('host: ')]
    if len(hosts) != 1 or os.environ.get('CARGO_BUILD_TARGET') not in (None, '', hosts[0]):
        raise ValueError('candidate compiler target must match the native host')
    return {'target': hosts[0], 'revision': revision, 'run_id': os.environ.get('GITHUB_RUN_ID', ''),
            'workspace': str(root.resolve()), 'os': platform.system(), 'arch': platform.machine(),
            'nextest': subprocess.check_output(['cargo-nextest', 'nextest', '--version'], text=True).strip(),
            'toolchain': compiler,
            'inputs': {name: digest(root / name) for name in ('Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml')},
            'profile': 'test', 'features': 'all',
            'environment': {name: os.environ.get(name, '') for name in ENVIRONMENT}}


def artifact(root, value):
    path = Path(value).resolve(strict=True)
    target = Path(os.environ.get('CARGO_TARGET_DIR', root / 'target'))
    if not target.is_absolute():
        target = root / target
    if not target.resolve().is_relative_to(root.resolve()) or not path.is_relative_to(target.resolve()) or not path.is_file() or not 0 < path.stat().st_size <= MAX_BINARY:
        raise ValueError(f'candidate executable must be a bounded file under its Cargo target directory: {path} (target {target.resolve()}, workspace {root.resolve()})')
    return {'path': str(path), 'sha256': digest(path)}


def seal(record):
    content = {key: value for key, value in record.items() if key != 'fingerprint'}
    return hashlib.sha256(json.dumps(content, sort_keys=True, separators=(',', ':')).encode()).hexdigest()


def record(root, revision, messages, destination, native, archive=None):
    entries = {}
    for message in messages:
        if message.get('reason') != 'compiler-artifact' or not message.get('executable'):
            continue
        target, profile = message['target'], message['profile']
        role = None
        if target['kind'] == ['bin'] and not profile['test'] and Path(target.get('src_path', root / 'src/main.rs')).resolve() == (root / 'src/main.rs').resolve():
            role = 'engine'
        if target['name'] == 'persistent_family' and target['kind'] == ['test']:
            role = 'native-store'
        if role:
            if role in entries:
                raise ValueError('duplicate candidate compiler artifact')
            entries[role] = artifact(root, message['executable'])
            entries[role]['compiler_profile'] = profile
            entries[role]['resolved_features'] = message['features']
    if 'engine' not in entries or native and 'native-store' not in entries:
        raise ValueError('candidate build did not produce the required executables')
    description = identity(root, revision)
    for entry in entries.values():
        verify_machine(Path(entry['path']), release_platform(description['target']))
    result = {'schema_version': 1, 'identity': description, 'artifacts': entries}
    if archive is not None:
        result['archive'] = {'name': archive.name, 'sha256': digest(archive)}
    result['fingerprint'] = seal(result)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(result, indent=2) + '\n')
    return result


def load(root, revision, manifest, fingerprint=None):
    if manifest.stat().st_size > 64 * 1024:
        raise ValueError('candidate manifest exceeds budget')
    result = json.loads(manifest.read_text())
    if result.get('schema_version') != 1 or result.get('fingerprint') != seal(result) or fingerprint is not None and result.get('fingerprint') != fingerprint:
        raise ValueError('candidate manifest fingerprint mismatch')
    if result['identity'] != identity(root, revision):
        raise ValueError('candidate revision, target, toolchain, profile, features or inputs changed')
    return result


def verify(root, revision, manifest, role, fingerprint=None):
    result = load(root, revision, manifest, fingerprint)
    if role not in ('engine', 'native-store') or role not in result['artifacts']:
        raise ValueError('requested candidate executable was not built')
    expected = result['artifacts'][role]
    actual = artifact(root, expected['path'])
    if actual['sha256'] != expected['sha256']:
        raise ValueError('candidate executable checksum mismatch')
    verify_machine(Path(actual['path']), release_platform(result['identity']['target']))
    return Path(actual['path'])


def extract(root, revision, manifest, fingerprint):
    result = load(root, revision, manifest, fingerprint)
    archive = manifest.parent / result['archive']['name']
    if result['archive']['name'] != 'candidate-tests.tar.zst' or not 0 < archive.stat().st_size <= 8 * 1024**3 or digest(archive) != result['archive']['sha256']:
        raise ValueError('candidate archive checksum, name or size mismatch')
    if (root / 'target').exists() or (root / 'target').is_symlink():
        raise ValueError('candidate extraction requires an absent Cargo target directory')
    # Extract to the identical workspace: existing env!(CARGO_BIN_EXE_*) paths
    # remain exact. Never relocate or relabel a binary built from another source.
    subprocess.run(['cargo-nextest', 'nextest', 'list', '--archive-file', str(archive),
                    '--extract-to', str(root), '--extract-overwrite', '--message-format', 'json',
                    '--list-type', 'binaries-only'], cwd=root, stdout=subprocess.DEVNULL, check=True)
    verify(root, revision, manifest, 'engine', fingerprint)


def run_tests(root, revision, manifest, group, split_docs, fingerprint=None, environment=None):
    verify(root, revision, manifest, 'engine', fingerprint)
    docs = ' | '.join(f'binary(={name})' for name in DOC_TESTS)
    filters = {'docs': docs, 'remaining': f'not({docs})' if split_docs else 'all()',
               'protocol': 'binary(=sparql_endpoints)',
               'native': 'binary(=persistent_family) & test(=native_rdf4j_family_publishes_from_restored_pinned_kg)'}
    if group not in filters:
        raise ValueError('unknown candidate test partition')
    command = ['cargo-nextest', 'nextest', 'run',
               '--binaries-metadata', str(root / 'target/nextest/binaries-metadata.json'),
               '--cargo-metadata', str(root / 'target/nextest/cargo-metadata.json'),
               '--no-fail-fast', '--no-tests', 'fail', '--test-threads', '2', '-E', filters[group]]
    if group == 'native':
        verify(root, revision, manifest, 'native-store', fingerprint)
        command += ['--run-ignored', 'only']
    subprocess.run(command, cwd=root, check=True, env=environment)
    verify(root, revision, manifest, 'engine', fingerprint)


def test_arguments(suite, native=True):
    if suite not in ('full', 'docs', 'sparql'):
        raise ValueError('unknown candidate test suite')
    if suite == 'full':
        return []
    names = ('sparql_endpoints',) if suite == 'sparql' else DOC_TESTS + (('sparql_endpoints',) if native else ())
    return [argument for name in names for argument in ('--test', name)]


def build(root, revision, suite, manifest, native, docs=False):
    identity(root, revision)
    if os.environ.get('CARGO_TARGET_DIR') not in (None, '', str(root / 'target')) or (root / 'target').exists() and not (root / 'target').resolve().is_relative_to(root.resolve()):
        raise ValueError('CI archives require the in-checkout Cargo target directory')
    if os.environ.get('CARGO_BUILD_BUILD_DIR') not in (None, '', str(root / 'target')):
        raise ValueError('CI archives require an in-checkout Cargo build directory')
    arguments = test_arguments(suite, native)
    if suite != 'full' and native:
        arguments += ['--test', 'persistent_family']
    manifest.parent.mkdir(parents=True, exist_ok=True)
    archive = manifest.parent / 'candidate-tests.tar.zst'
    command = ['cargo', 'nextest', 'archive', '--locked', '--all-features',
               '--cargo-profile', 'test', '--cargo-message-format', 'json',
               '--archive-file', str(archive), *arguments]
    print('Building candidate once: ' + ' '.join(command), flush=True)
    # Bounded Cargo metadata stream; retain only executable artifact records.
    process = subprocess.Popen(command, cwd=root, stdout=subprocess.PIPE)
    messages, total = [], 0
    try:
        while line := process.stdout.readline(1024 * 1024 + 1):
            total += len(line)
            if len(line) > 1024 * 1024 or total > 16 * 1024 * 1024:
                raise ValueError('candidate compiler metadata exceeds budget')
            message = json.loads(line)
            if message.get('reason') == 'compiler-message':
                rendered = message.get('message', {}).get('rendered', '')
                if rendered:
                    print(rendered[:65536], file=sys.stderr)
                    if len(rendered) > 65536:
                        print('[compiler diagnostic truncated at 64 KiB]', file=sys.stderr)
            if message.get('reason') == 'compiler-artifact' and message.get('executable'):
                if len(messages) >= 512:
                    raise ValueError('candidate executable count exceeds budget')
                messages.append(message)
        if process.wait() != 0:
            raise ValueError('candidate compilation failed')
    except BaseException:
        process.kill()
        process.wait()
        raise
    finally:
        process.stdout.close()
    if docs or suite == 'docs':
        observed = {m['target']['name'] for m in messages if m['profile']['test']}
        missing = set(DOC_TESTS) - observed
        if missing:
            raise ValueError('required documentation harnesses were not built: ' + ', '.join(sorted(missing)))
    if suite == 'full':
        # Rustdoc tests are a separate required harness, not a second CLI build.
        subprocess.run(['cargo', 'test', '--locked', '--all-features', '--doc'], cwd=root, check=True)
    if not 0 < archive.stat().st_size <= 8 * 1024**3:
        raise ValueError('candidate archive exceeds size budget')
    result = record(root, revision, messages, manifest, native, archive)
    print('Candidate fingerprint: ' + result['fingerprint'], flush=True)
    if 'GITHUB_OUTPUT' in os.environ:
        with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
            output.write('fingerprint=' + result['fingerprint'] + '\n')


def execute(root, revision, manifest, role, arguments, fingerprint=None):
    binary = verify(root, revision, manifest, role, fingerprint)
    subprocess.run([str(binary), *arguments], cwd=root, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=('build', 'extract', 'test', 'run'))
    parser.add_argument('--suite', choices=('full', 'docs', 'sparql'), default='full')
    parser.add_argument('--manifest', type=Path, default=Path('output/ci-engine.json'))
    parser.add_argument('--revision', required=True)
    parser.add_argument('--fingerprint')
    parser.add_argument('--group', choices=('docs', 'remaining', 'protocol', 'native'), default='remaining')
    parser.add_argument('--split-docs', choices=('true', 'false'), default='false')
    parser.add_argument('--native', choices=('true', 'false'), default='true')
    # Split the engine's arguments explicitly; never interpret them as shell code.
    before, after = sys.argv[1:], []
    if '--' in before:
        position = before.index('--')
        before, after = before[:position], before[position + 1:]
    args = parser.parse_args(before)
    if after and args.operation != 'run':
        parser.error('only run accepts executable arguments')
    root = Path(__file__).resolve().parents[1]
    if args.operation == 'build':
        build(root, args.revision, args.suite, args.manifest, args.native == 'true', args.split_docs == 'true')
    elif args.operation == 'extract':
        if not args.fingerprint:
            parser.error('artifact consumers require the producer fingerprint')
        extract(root, args.revision, args.manifest, args.fingerprint)
    elif args.operation == 'test':
        if not args.fingerprint:
            parser.error('artifact consumers require the producer fingerprint')
        run_tests(root, args.revision, args.manifest, args.group, args.split_docs == 'true', args.fingerprint)
    else:
        if not args.fingerprint:
            parser.error('artifact consumers require the producer fingerprint')
        execute(root, args.revision, args.manifest, 'engine', after, args.fingerprint)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f'GraphArch candidate: {error}', file=sys.stderr)
        sys.exit(1)
