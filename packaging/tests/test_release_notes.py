"""Breaking commits must lead the generated GitHub release notes."""
import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).parents[1]))
from release_notes import BreakingChange, ReleaseNotes, previous_tag
from auto_release import select_bump

ROOT = Path(__file__).resolve().parents[2]

BANG = ('feat(publication)!: name PDF editions from a mandatory edition input (#94)\n\n'
        '* feat(publication)!: name PDF editions from a mandatory edition input\n\nBody.')
FOOTER = ('feat(publication): publish RDF through an RDF renderer (#101)\n\nDetails.\n\n'
          'BREAKING CHANGE: ga:Download is removed.\nUse ga:RdfRenderer targets.\n\n'
          'Co-authored-by: Someone <someone@example.org>')
SQUASHED = ('feat(diagrams): add diagrams (#7)\n\n* feat(diagrams)!: require PlantUML\n\n'
            '* fix(diagrams): tidy\n\nBREAKING-CHANGE: Java is required.')
PLAIN = 'fix(render): retry file removal (#132)\n\nNot breaking; mentions BREAKING CHANGE inline.'


class ReleaseNotesTest(unittest.TestCase):
    def test_bang_header_is_breaking(self):
        change = BreakingChange.parse(BANG)
        self.assertEqual(change.subject, 'feat(publication)!: name PDF editions from a mandatory edition input (#94)')
        self.assertEqual(change.details, ())

    def test_footer_is_breaking_and_keeps_its_continuation(self):
        change = BreakingChange.parse(FOOTER)
        self.assertEqual(change.details, ('ga:Download is removed. Use ga:RdfRenderer targets.',))

    def test_squashed_inner_bang_and_hyphenated_footer_are_breaking(self):
        change = BreakingChange.parse(SQUASHED)
        self.assertEqual(change.details, ('feat(diagrams)!: require PlantUML', 'Java is required.'))

    def test_ordinary_commit_is_not_breaking(self):
        self.assertIsNone(BreakingChange.parse(PLAIN))

    def test_breaking_section_comes_first(self):
        body = ReleaseNotes.from_messages([PLAIN, FOOTER, BANG]).markdown()
        self.assertTrue(body.startswith('## Breaking changes\n'))
        self.assertIn('- feat(publication): publish RDF through an RDF renderer (#101)\n'
                      '  - ga:Download is removed. Use ga:RdfRenderer targets.\n', body)
        self.assertIn('- feat(publication)!: name PDF editions', body)
        self.assertNotIn('retry file removal', body)
        self.assertIn('/usage/migration/', body)

    def test_release_without_breaking_commits_has_no_section(self):
        self.assertEqual(ReleaseNotes.from_messages([PLAIN]).markdown(), '')

    def test_previous_tag_is_the_highest_lower_release(self):
        tags = ['v0.2.9', 'v0.2.10', 'v0.2.31', 'v0', 'v0.3.0-rc1', 'v0.2.32']
        self.assertEqual(previous_tag('0.2.32', tags), 'v0.2.31')
        self.assertIsNone(previous_tag('0.1.0', tags))

    def test_breaking_commit_does_not_escalate_the_automatic_bump(self):
        # Existing policy: patch by default, minor only by explicit trailer, never major.
        self.assertEqual(select_bump(FOOTER, 'auto'), 'patch')
        self.assertEqual(select_bump(BANG + '\n\nGraphArch-Release-Bump: minor', 'auto'), 'minor')

    def test_github_release_prepends_the_breaking_section(self):
        workflow = (ROOT / '.github/workflows/distribute.yml').read_text(encoding='utf-8')
        self.assertIn('packaging/release_notes.py', workflow)
        self.assertIn('body_path: ${{ runner.temp }}/release-notes.md', workflow)
        self.assertIn('generate_release_notes: true', workflow)


if __name__ == '__main__':
    unittest.main()
