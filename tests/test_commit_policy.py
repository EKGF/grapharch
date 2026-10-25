"""Commit checks cover introduced history, including branch creation."""
import pathlib
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]


class CommitRangeTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = pathlib.Path(self.temporary.name)
        self.git("init", "-q")
        self.git("config", "user.name", "Test")
        self.git("config", "user.email", "test@example.org")
        self.git("-c", "commit.gpgsign=false", "commit", "--allow-empty", "-qm", "historical malformed message")
        self.base = self.git("rev-parse", "HEAD").strip()
        self.git("update-ref", "refs/remotes/origin/main", self.base)
        self.git("-c", "commit.gpgsign=false", "commit", "--allow-empty", "-qm", "fix: validate new work")
        self.head = self.git("rev-parse", "HEAD").strip()

    def git(self, *args):
        return subprocess.check_output(["git", "-C", str(self.directory), *args], text=True)

    def script(self, event, before="", base=""):
        import os
        environment = {**os.environ, "GRAPHARCH_EVENT": event, "GRAPHARCH_HEAD": self.head,
                       "GRAPHARCH_BEFORE": before, "GRAPHARCH_BASE": base,
                       "GRAPHARCH_DEFAULT_BRANCH": "main"}
        environment.pop("GITHUB_OUTPUT", None)
        return subprocess.run(["python3", str(ROOT / ".github/scripts/commit-range.py")],
                              cwd=self.directory, env=environment, text=True, capture_output=True)

    def test_push_excludes_already_published_malformed_history(self):
        result = self.script("push", before=self.base)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), f"{self.base}..{self.head}")

    def test_pull_request_uses_its_explicit_base(self):
        result = self.script("pull_request", base=self.base)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), f"{self.base}..{self.head}")

    def test_new_branch_uses_a_verified_default_branch_merge_base(self):
        result = self.script("push", before="0" * 40)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), f"{self.base}..{self.head}")

    def test_unknown_base_and_injected_revision_are_hard_errors(self):
        for before in ("f" * 40, "HEAD;echo invalid", ""):
            result = self.script("push", before=before)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, "")
