"""Candidate source must have one Linux compiler owner and safe deployment."""
from pathlib import Path
import unittest
import re

ROOT = Path(__file__).resolve().parents[2]


class CandidateWorkflows(unittest.TestCase):
    def test_only_candidate_workflow_automatically_builds_source(self):
        for name in ('pages', 'sparql-integration'):
            workflow = (ROOT / f'.github/workflows/{name}.yml').read_text()
            self.assertNotIn('\n  push:', workflow, name)
            self.assertNotIn('\n  pull_request:', workflow, name)
            self.assertNotIn('cargo ', workflow, name)
            self.assertIn('uses: ./.github/workflows/rust-ci.yml', workflow)
        workflow = (ROOT / '.github/workflows/rust-ci.yml').read_text()
        self.assertEqual(workflow.count('packaging/ci_candidate.py build'), 1)
        self.assertNotIn('cargo run', workflow)
        self.assertIn('os: [windows-latest, macos-latest]', workflow)
        self.assertIn('packaging/ci_native_store.py', workflow)
        self.assertIn('GRAPHARCH_CI_GROUP:', workflow)
        self.assertIn("--group docs", workflow)
        self.assertIn('GRAPHARCH_CI_SPLIT_DOCS', workflow)
        self.assertIn('GRAPHARCH_CI_ARCHIVE_SMOKE', workflow)
        self.assertIn('test_ci_transport.py', workflow)
        for job in ('compile', 'linux-tests', 'native-store', 'docs'):
            block = workflow.split('\n  ' + job + ':', 1)[1]
            # Stop at the next job, not at nested YAML fields.
            block = re.split(r'\n  [a-z][a-z-]*:', block, maxsplit=1)[0]
            self.assertIn('    permissions:\n      contents: read', block)
            self.assertNotIn('pages: write', block)
        consumer = (ROOT / '.github/actions/setup-ci-consumer/action.yml').read_text()
        self.assertNotIn('cargo build', consumer)
        self.assertIn('fingerprint', consumer)
        self.assertIn("cache: 'false'", consumer)
        self.assertIn("github.event_name != 'pull_request'", workflow)
        self.assertIn("github.ref == 'refs/heads/main'", workflow)

    def test_docs_schedule_cannot_cancel_full_candidate_checks(self):
        workflow = (ROOT / '.github/workflows/rust-ci.yml').read_text()
        self.assertIn("inputs.scope || 'candidate'", workflow.split('jobs:', 1)[0])

    def test_release_identity_is_explicitly_forwarded_into_native_containers(self):
        workflow=(ROOT / '.github/workflows/distribute.yml').read_text()
        self.assertIn('GRAPHARCH_BUILD_REVISION: ${{ needs.metadata.outputs.revision }}', workflow)
        self.assertIn('GRAPHARCH_BUILD_RELEASE_VERSION: ${{ needs.metadata.outputs.version }}', workflow)
        self.assertIn('docker-options: --env GRAPHARCH_BUILD_REVISION --env GRAPHARCH_BUILD_RELEASE_VERSION',workflow)

    def test_release_still_extracts_one_native_binary_per_target(self):
        workflow = (ROOT / '.github/workflows/distribute.yml').read_text()
        self.assertEqual(workflow.count('command: build'), 1)
        self.assertIn('--release --locked --bindings bin', workflow)
        self.assertIn('Create standalone and npm artifacts from the wheel', workflow)
        self.assertNotIn('cargo build', workflow)
