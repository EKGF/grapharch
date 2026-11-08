"""A real archive round trip must work without Cargo on the consumer PATH."""
import os
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))
import ci_candidate as candidate


@unittest.skipUnless(os.environ.get('GRAPHARCH_CI_ARCHIVE_SMOKE') == '1', 'run by the compiler job after pinned nextest installation')
class ArchiveTransport(unittest.TestCase):
    def test_original_fixture_survives_target_deletion_and_runs_without_cargo(self):
        with tempfile.TemporaryDirectory() as temporary, tempfile.TemporaryDirectory() as tools:
            root = Path(temporary).resolve()
            (root / 'src').mkdir(); (root / 'tests').mkdir()
            (root / 'Cargo.toml').write_text('[package]\nname="ci-archive-probe"\nversion="0.0.0"\nedition="2024"\n')
            (root / 'rust-toolchain.toml').write_bytes((Path(__file__).parents[2] / 'rust-toolchain.toml').read_bytes())
            (root / '.gitignore').write_text('target/\noutput/\n')
            (root / 'src/lib.rs').write_text('/// ```\n/// assert_eq!(ci_archive_probe::answer(), 42);\n/// ```\npub fn answer() -> u8 {42}\n')
            (root / 'src/main.rs').write_text('fn main() { println!("original archive fixture"); }\n')
            (root / 'tests/persistent_family.rs').write_text('''#[test]
fn compile_time_cli_path_is_restored() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_ci-archive-probe")).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "original archive fixture");
}
''')
            environment = {'CARGO_TARGET_DIR': str(root / 'target'), 'CARGO_BUILD_BUILD_DIR': str(root / 'target'), 'GITHUB_OUTPUT': str(root / 'output/outputs')}
            with patch.dict(os.environ, environment):
                subprocess.run(['cargo', 'generate-lockfile', '--offline'], cwd=root, check=True)
                subprocess.run(['git', 'init', '--quiet'], cwd=root, check=True)
                subprocess.run(['git', 'add', '.'], cwd=root, check=True)
                subprocess.run(['git', '-c', 'user.name=CI Fixture', '-c', 'user.email=ci@example.invalid',
                                '-c', 'commit.gpgsign=false', 'commit', '--quiet', '-m', 'test(ci): archive transport fixture'], cwd=root, check=True)
                revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip()
                os.environ['GRAPHARCH_BUILD_REVISION'] = revision
                manifest = root / 'output/ci-engine.json'
                candidate.build(root, revision, 'full', manifest, True)
                fingerprint = json.loads(manifest.read_text())['fingerprint']
                shutil.rmtree(root / 'target')
                for tool in ('git', 'rustc', 'cargo-nextest', 'sed', 'head'):
                    Path(tools, tool).symlink_to(shutil.which(tool))
                with patch.dict(os.environ, {'PATH': tools}):
                    self.assertIsNone(shutil.which('cargo'))
                    candidate.extract(root, revision, manifest, fingerprint)
                    candidate.run_tests(root, revision, manifest, 'remaining', False, fingerprint)
                    candidate.execute(root, revision, manifest, 'engine', [], fingerprint)
