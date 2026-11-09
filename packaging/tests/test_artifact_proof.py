"""Supported-trigger publication may reuse only the exact verified native distribution."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).parents[1]))
from distribute import PLATFORMS
from artifact_proof import validate_run, validate_directory


class ArtifactProof(unittest.TestCase):
    def run_record(self):
        return {'path': '.github/workflows/automatic-release.yml', 'event': 'workflow_run',
                'head_branch': 'main', 'head_sha': 'a' * 40,
                'head_repository': {'full_name': 'EKGF/grapharch'}, 'conclusion': 'failure'}

    def jobs(self):
        names = ['reserve', 'distribute / metadata', 'distribute / verify-proof',
                 'distribute / assemble', 'distribute / github-release']
        names += ['distribute / binaries (' + item['target'] + ', native)' for item in PLATFORMS]
        return [{'id': index, 'name': name, 'conclusion': 'success'} for index, name in enumerate(names)]

    def test_registry_failure_does_not_discard_successful_immutable_native_outputs(self):
        validate_run(self.run_record(), self.jobs(), 'EKGF/grapharch', 'a' * 40)
        for key, value in [('event', 'pull_request'), ('head_sha', 'b' * 40),
                           ('head_branch', 'feature/untrusted'), ('path', '.github/workflows/other.yml'),
                           ('head_repository', {'full_name': 'attacker/grapharch'})]:
            record = dict(self.run_record(), **{key: value})
            with self.assertRaises(ValueError):
                validate_run(record, self.jobs(), 'EKGF/grapharch', 'a' * 40)

    def test_native_or_assembly_failure_cannot_be_hidden_by_outer_success(self):
        for index in range(len(self.jobs())):
            jobs = self.jobs()
            jobs[index]['conclusion'] = 'skipped'
            with self.assertRaises(ValueError):
                validate_run(self.run_record(), jobs, 'EKGF/grapharch', 'a' * 40)

    def test_manifest_source_and_version_must_match_release_checkout(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for item in PLATFORMS:
                (root / f'grapharch-{item["target"]}.json').write_text(json.dumps({
                    'source_commit': 'b' * 40, 'version': '0.2.5', 'target': item['target']}))
            validate_directory(root, 'b' * 40, '0.2.5')
            with self.assertRaises(ValueError):
                validate_directory(root, 'c' * 40, '0.2.5')
            with self.assertRaises(ValueError):
                validate_directory(root, 'b' * 40, '0.2.6')

    def test_workflow_run_builds_then_dispatches_the_same_publisher_without_recompiling(self):
        root = Path(__file__).parents[2]
        coordinator = (root / '.github/workflows/automatic-release.yml').read_text()
        self.assertIn("publish-packages: ${{ github.event_name == 'workflow_dispatch' }}", coordinator)
        self.assertIn('gh workflow run automatic-release.yml', coordinator)
        self.assertIn('publish-run=$GITHUB_RUN_ID', coordinator)
        self.assertIn("'build' || 'publish'", coordinator)
        workflow = (root / '.github/workflows/distribute.yml').read_text()
        native = workflow.split('  binaries:\n', 1)[1].split('  verify-artifacts:\n', 1)[0]
        self.assertIn("inputs.artifact-run == ''", native)
        self.assertIn('run-id: ${{ inputs.artifact-run }}', workflow)
        self.assertIn('name: release-distribution', workflow)
        self.assertIn('packaging/artifact_proof.py', workflow)
        self.assertIn('--output "$GITHUB_WORKSPACE/dist"', workflow)
