"""Build numbers describe the compiler job, never a later artifact consumer."""
import json
import os
from pathlib import Path
import sys
import unittest
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).parents[1]))
from build_context import BuildContext, prepared_context, release_context

REVISION = 'a' * 40
ENVIRONMENT = {'GITHUB_ACTIONS':'true','GITHUB_SHA':REVISION,'GITHUB_REPOSITORY':'EKGF/grapharch',
    'GITHUB_WORKFLOW_REF':'EKGF/grapharch/.github/workflows/rust-ci.yml@refs/heads/main',
    'GITHUB_RUN_ID':'12345','GITHUB_RUN_NUMBER':'321','GITHUB_RUN_ATTEMPT':'1'}

class ContextTests(unittest.TestCase):
    def test_release_child_requires_proof_and_retains_actual_controller_identity(self):
        with patch.dict('os.environ', ENVIRONMENT, clear=True):
            with patch('build_context.verify_release') as verify:
                context = release_context('b' * 40, REVISION, 42)
                verify.assert_called_once_with('EKGF/grapharch', 42, 'b' * 40, REVISION)
                self.assertEqual(context.revision, 'b' * 40)
                self.assertEqual(context.run_number, 321)
                self.assertEqual(context.run_attempt, 1)
            with patch('build_context.verify_release', side_effect=ValueError('unverified')):
                with self.assertRaisesRegex(ValueError, 'unverified'):
                    release_context('b' * 40, REVISION, 42)

    def test_only_compiler_jobs_prepare_identity_and_release_container_preserves_it(self):
        root = Path(__file__).parents[2]
        workflow = (root / '.github/workflows/rust-ci.yml').read_text()
        self.assertEqual(workflow.count('packaging/build_context.py --revision'), 2)
        consumers = workflow[workflow.index('  source-tests:'):] if '  source-tests:' in workflow else workflow[workflow.index('    name: Linux source tests'):]
        consumers = consumers[:consumers.index('    name: Format, Lint, Test')]
        self.assertNotIn('packaging/build_context.py', consumers)
        release = (root / '.github/workflows/distribute.yml').read_text()
        self.assertIn('--env GRAPHARCH_BUILD_CONTEXT', release)
        self.assertEqual(release.count('packaging/build_context.py --revision'), 2)

    def test_actual_producer_is_bound_to_source_workflow_and_build(self):
        with patch.dict('os.environ', ENVIRONMENT, clear=True):
            context=BuildContext.current(REVISION)
            self.assertEqual(context.run_number,321)
            self.assertEqual(context.run_attempt,1)
            self.assertEqual(BuildContext.parse(context.json()),context)
            with patch.dict('os.environ',{'GRAPHARCH_BUILD_CONTEXT':context.json()}):
                self.assertEqual(prepared_context(REVISION),context)
            with self.assertRaises(ValueError): BuildContext.current('b'*40)
            with patch.dict(os.environ, {'GITHUB_WORKFLOW_REF':'EKGF/grapharch/.github/workflows/pages.yml@refs/heads/main'}):
                self.assertIn('pages.yml', BuildContext.current(REVISION).workflow_ref)

    def test_retry_consumes_original_producer_identity_without_renumbering(self):
        with patch.dict('os.environ',ENVIRONMENT,clear=True):
            producer=BuildContext.current(REVISION)
            with patch.dict('os.environ',{'GITHUB_RUN_ATTEMPT':'2'}):
                producer.validate_consumer(REVISION)
                self.assertEqual(producer.run_attempt,1)
                for name,value in [('GITHUB_RUN_NUMBER','322'),('GITHUB_RUN_ID','54321'),('GITHUB_REPOSITORY','other/repo'),('GITHUB_RUN_ATTEMPT','0')]:
                    with self.subTest(name=name),patch.dict('os.environ',{name:value}),self.assertRaises(ValueError):
                        producer.validate_consumer(REVISION)

    def test_ambient_values_and_malformed_context_do_not_claim_a_ci_build(self):
        with patch.dict('os.environ',{'GITHUB_RUN_NUMBER':'321'},clear=True):
            self.assertIsNone(prepared_context(REVISION))
            with self.assertRaises(ValueError): BuildContext.current(REVISION)
        with patch.dict('os.environ',ENVIRONMENT,clear=True):
            good=json.loads(BuildContext.current(REVISION).json())
            for field,value in [('run_number',True),('run_attempt',0),('run_id',2**64),('revision','b'*40+'\n'),('repository','../..'),('workflow_ref','unrelated/.github/workflows/rust-ci.yml@refs/heads/main')]:
                bad=dict(good,**{field:value})
                with self.subTest(field=field),self.assertRaises(ValueError): BuildContext.parse(bad)
            with self.assertRaises(ValueError): BuildContext.parse(dict(good,unknown='ignored'))
            with patch.dict('os.environ',{'GRAPHARCH_BUILD_CONTEXT':json.dumps(dict(good,revision='b'*40))}),self.assertRaises(ValueError):
                prepared_context(REVISION)
