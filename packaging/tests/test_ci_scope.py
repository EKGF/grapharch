import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('ci_scope', Path(__file__).parents[1] / 'ci_scope.py')
scope = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scope)


class ScopeTests(unittest.TestCase):
    def test_docs_only_preserves_linux_docs_and_endpoint_checks(self):
        self.assertEqual(scope.select('candidate', 'push', 'refs/heads/main', ['docs/usage/index.md']),
                         {'desktop': False, 'docs': True, 'sparql': True, 'suite': 'docs'})

    def test_embedded_ontology_and_mixed_edits_require_three_operating_systems(self):
        for paths in (['docs/other/ontology/publication.ttl'], ['docs/a.md', 'src/main.rs'], None):
            self.assertTrue(scope.select('candidate', 'push', 'refs/heads/main', paths)['desktop'])

    def test_feature_branch_does_not_implicitly_publish_docs(self):
        result = scope.select('candidate', 'push', 'refs/heads/feature/x', ['src/main.rs'])
        self.assertTrue(result['desktop'])
        self.assertFalse(result['docs'])
        self.assertTrue(scope.select('candidate', 'pull_request', 'refs/pull/1/merge', ['src/main.rs'])['docs'])

    def test_manual_and_scheduled_workflows_have_explicit_linux_only_scope(self):
        self.assertEqual(scope.select('docs', 'schedule', 'refs/heads/main', None),
                         {'desktop': False, 'docs': True, 'sparql': False, 'suite': 'docs'})
        self.assertEqual(scope.select('sparql', 'workflow_dispatch', 'refs/heads/main', None),
                         {'desktop': False, 'docs': False, 'sparql': True, 'suite': 'sparql'})
        with self.assertRaises(ValueError):
            scope.select('latest', 'push', 'refs/heads/main', [])
