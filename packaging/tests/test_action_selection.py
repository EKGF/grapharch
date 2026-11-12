"""The Action installs the engine its own exact source identifies."""
import importlib.util
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parents[1]))
SPEC = importlib.util.spec_from_file_location(
    'action_selection', Path(__file__).parents[1] / 'action_selection.py')
selection = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(selection)

RELEASE = '595e55913096cb7f39633189ef67f5d916ef4dcc'
ROOT = Path(__file__).resolve().parents[2]


def git(root, *arguments):
    return subprocess.check_output(['git', '-C', str(root), *arguments], text=True).strip()


def committed_checkout(root, version='0.2.8'):
    subprocess.run(['git', 'init', '-q', str(root)], check=True)
    (root / 'Cargo.toml').write_text(f'[package]\nname = "grapharch"\nversion = "{version}"\n')
    git(root, 'add', '.')
    git(root, '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.org',
        'commit', '-qm', 'fixture')
    return git(root, 'rev-parse', 'HEAD')


class UnconsultedCheckout:
    """Explicit selections must not depend on the Action's own source."""

    def version(self):
        raise AssertionError('explicit selection consulted the action version')

    def commit(self):
        raise AssertionError('explicit selection consulted the action commit')

    def require_commit(self, revision):
        raise AssertionError('explicit release selection inspected the action checkout')


class OwnRelease(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / 'Cargo.toml').write_text('[package]\nname = "grapharch"\nversion = "0.2.8"\n')

    def test_release_commit_reference_selects_its_own_version_and_commit(self):
        checkout = selection.ActionCheckout(self.root, 'EKGF/grapharch', RELEASE)
        chosen = selection.EngineSelection.resolve('', '', '', checkout)
        self.assertEqual((chosen.kind, chosen.version, chosen.revision),
                         ('release', '0.2.8', RELEASE))

    def test_repository_name_comparison_ignores_case_only(self):
        for repository in ('ekgf/grapharch', 'EKGF/GraphArch'):
            with self.subTest(repository=repository):
                checkout = selection.ActionCheckout(self.root, repository, RELEASE)
                self.assertEqual(checkout.commit(), RELEASE)

    def test_references_that_are_not_exact_commits_fail(self):
        for reference in ('main', 'v0.2.8', 'v0', 'v0.2', RELEASE[:12], RELEASE.upper(),
                          '0' * 40, 'refs/heads/main'):
            checkout = selection.ActionCheckout(self.root, 'EKGF/grapharch', reference)
            with self.subTest(reference=reference), \
                    self.assertRaisesRegex(ValueError, 'exact release commit SHA'):
                selection.EngineSelection.resolve('', '', '', checkout)

    def test_a_different_repository_cannot_identify_this_action(self):
        checkout = selection.ActionCheckout(self.root, 'someone/wrapper', RELEASE)
        with self.assertRaisesRegex(ValueError, 'someone/wrapper'):
            selection.EngineSelection.resolve('', '', '', checkout)

    def test_inexact_checkout_version_fails(self):
        (self.root / 'Cargo.toml').write_text('[package]\nversion = "0.2.8-dev"\n')
        checkout = selection.ActionCheckout(self.root, 'EKGF/grapharch', RELEASE)
        with self.assertRaisesRegex(ValueError, 'MAJOR.MINOR.PATCH'):
            selection.EngineSelection.resolve('', '', '', checkout)


class LocalCheckout(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name) / 'grapharch tool'
        self.root.mkdir()

    def test_local_action_identifies_its_clean_checked_out_commit(self):
        head = committed_checkout(self.root)
        chosen = selection.EngineSelection.resolve(
            '', '', '', selection.ActionCheckout(self.root, '', ''))
        self.assertEqual((chosen.kind, chosen.version, chosen.revision), ('release', '0.2.8', head))

    def test_modified_local_checkout_has_no_exact_identity(self):
        committed_checkout(self.root)
        (self.root / 'Cargo.toml').write_text('[package]\nversion = "0.2.9"\n')
        with self.assertRaisesRegex(ValueError, 'clean'):
            selection.ActionCheckout(self.root, '', '').commit()

    def test_directory_inside_another_repository_is_not_an_action_checkout(self):
        committed_checkout(self.root)
        nested = self.root / 'copy'
        nested.mkdir()
        (nested / 'Cargo.toml').write_text('[package]\nversion = "0.2.8"\n')
        git(self.root, 'add', '.')
        git(self.root, '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.org',
            'commit', '-qm', 'nested copy')
        with self.assertRaisesRegex(ValueError, 'root of a Git checkout'):
            selection.ActionCheckout(nested, '', '').commit()

    def test_directory_without_git_has_no_identity(self):
        (self.root / 'Cargo.toml').write_text('[package]\nversion = "0.2.8"\n')
        with patch.dict(os.environ, {'GIT_CEILING_DIRECTORIES': str(self.root.parent)}), \
                self.assertRaisesRegex(ValueError, 'root of a Git checkout'):
            selection.ActionCheckout(self.root, '', '').commit()

    def test_reference_without_repository_is_rejected(self):
        committed_checkout(self.root)
        with self.assertRaisesRegex(ValueError, 'repository'):
            selection.ActionCheckout(self.root, '', RELEASE).commit()


class ExplicitSelections(unittest.TestCase):
    def test_explicit_release_needs_both_version_and_expected_revision(self):
        for version, expected in (('0.2.8', ''), ('', RELEASE)):
            with self.subTest(version=version, expected=expected), \
                    self.assertRaisesRegex(ValueError, 'together'):
                selection.EngineSelection.resolve(version, expected, '', UnconsultedCheckout())
        chosen = selection.EngineSelection.resolve('0.2.8', RELEASE, '', UnconsultedCheckout())
        self.assertEqual((chosen.kind, chosen.version, chosen.revision),
                         ('release', '0.2.8', RELEASE))

    def test_explicit_release_values_are_exact(self):
        for version, expected in (('latest', RELEASE), ('v0.2.8', RELEASE),
                                  ('0.2.8', 'main'), ('0.2.8', '0' * 40)):
            with self.subTest(version=version, expected=expected), self.assertRaises(ValueError):
                selection.EngineSelection.resolve(version, expected, '', UnconsultedCheckout())

    def test_source_revision_excludes_release_selection_and_requires_its_checkout(self):
        for version, expected in (('0.2.8', ''), ('', RELEASE), ('0.2.8', RELEASE)):
            with self.subTest(version=version, expected=expected), \
                    self.assertRaisesRegex(ValueError, 'source-revision'):
                selection.EngineSelection.resolve(version, expected, RELEASE, UnconsultedCheckout())
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            head = committed_checkout(root, '0.3.0')
            chosen = selection.EngineSelection.resolve(
                '', '', head, selection.ActionCheckout(root, '', ''))
            self.assertEqual((chosen.kind, chosen.version, chosen.revision), ('source', '0.3.0', head))
            with self.assertRaisesRegex(ValueError, 'does not match source-revision'):
                selection.EngineSelection.resolve(
                    '', '', 'a' * 40, selection.ActionCheckout(root, '', ''))
        for revision in ('main', '0' * 40):
            with self.subTest(revision=revision), self.assertRaises(ValueError):
                selection.EngineSelection.resolve('', '', revision, UnconsultedCheckout())


class ActionStep(unittest.TestCase):
    def test_step_publishes_the_selected_engine_from_its_environment(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'Cargo.toml').write_text('[package]\nversion = "0.2.8"\n')
            outputs = root / 'outputs'
            environment = {
                'GITHUB_ACTION_PATH': str(root), 'GITHUB_OUTPUT': str(outputs),
                'GRAPHARCH_ACTION_REPOSITORY': 'EKGF/grapharch', 'GRAPHARCH_ACTION_REF': RELEASE,
                'GRAPHARCH_VERSION': '', 'GRAPHARCH_EXPECTED_REVISION': '',
                'GRAPHARCH_SOURCE_REVISION': ''}
            with patch.dict(os.environ, environment):
                selection.main()
            self.assertEqual(outputs.read_text(),
                             f'kind=release\nversion=0.2.8\nrevision={RELEASE}\n')


class ActionMetadata(unittest.TestCase):
    def steps(self):
        action = (ROOT / 'action.yml').read_text()
        body = action.split('\nruns:\n', 1)[1].split('\n  steps:\n', 1)[1]
        # Comment lines between steps belong to no step.
        body = re.sub(r'(?m)^ *#.*\n', '', body)
        return [step for step in re.split(r'(?m)^    - ', body) if step.strip()]

    def test_no_selector_is_required_so_the_reference_alone_selects_the_release(self):
        inputs = (ROOT / 'action.yml').read_text().split('\noutputs:\n', 1)[0]
        for name in ('version', 'expected-revision', 'source-revision'):
            block = inputs.split(f'\n  {name}:\n', 1)[1].split('\n  ', 1)[0]
            self.assertNotIn('required: true', block, name)

    def test_own_reference_is_captured_before_any_nested_action_runs(self):
        steps = self.steps()
        first = steps[0]
        self.assertTrue(first.startswith('id: identity'), first)
        self.assertNotIn('uses:', first)
        self.assertIn('${{ github.action_repository }}', first.split('run:', 1)[0])
        self.assertIn('${{ github.action_ref }}', first.split('run:', 1)[0])
        for step in steps[1:]:
            self.assertNotIn('github.action_ref', step)
            self.assertNotIn('github.action_repository', step)

    def test_release_steps_use_only_the_resolved_selection(self):
        steps = self.steps()
        select = next(step for step in steps if step.startswith('id: select'))
        for name in ('version', 'expected-revision', 'source-revision'):
            # Only selection reads the selector inputs; later steps cannot reinterpret them.
            self.assertEqual(sum(f'inputs.{name} ' in step for step in steps), 1, name)
            self.assertIn(f'inputs.{name} ', select)
        install = next(step for step in steps if step.startswith('id: install'))
        self.assertIn("steps.select.outputs.kind == 'release'", install)
        self.assertIn('GRAPHARCH_VERSION: ${{ steps.select.outputs.version }}', install)
        self.assertIn('GRAPHARCH_EXPECTED_REVISION: ${{ steps.select.outputs.revision }}', install)

    def test_action_scripts_never_write_bytecode_into_the_identified_checkout(self):
        action = (ROOT / 'action.yml').read_text()
        invocations = re.findall(r'python[^\n]*\$GITHUB_ACTION_PATH', action)
        self.assertTrue(invocations)
        for invocation in invocations:
            self.assertTrue(invocation.startswith('python -B "'), invocation)


if __name__ == '__main__':
    unittest.main()
