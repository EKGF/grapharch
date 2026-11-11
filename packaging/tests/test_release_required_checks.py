"""Every workflow that authorizes a release runs on every main push."""
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).parents[2]
sys.path.insert(0, str(ROOT / 'packaging'))
from auto_release import REQUIRED_PUSH_CHECKS


def workflow(name):
    matches = [path for path in sorted((ROOT / '.github/workflows').glob('*.yml'))
               if f'name: {name}' in path.read_text().splitlines()[:1]]
    if len(matches) != 1:
        raise AssertionError(f'expected one workflow named {name}, found {matches}')
    return matches[0].read_text().splitlines()


def push_trigger(lines):
    """Return the inline trigger list or the indented settings of on.push."""
    start = lines.index(next(line for line in lines if line.startswith('on:')))
    inline = lines[start].removeprefix('on:').strip()
    if inline:
        return inline, []
    for index in range(start + 1, len(lines)):
        if not lines[index].startswith(' '):
            break
        if lines[index].strip() == 'push:':
            settings = []
            for line in lines[index + 1:]:
                if len(line) - len(line.lstrip()) <= len(lines[index]) - len(lines[index].lstrip()):
                    break
                settings.append(line.strip())
            return 'push', settings
    raise AssertionError('workflow has no push trigger')


class RequiredChecks(unittest.TestCase):
    def test_release_checks_run_on_every_main_push(self):
        for name in REQUIRED_PUSH_CHECKS:
            with self.subTest(workflow=name):
                trigger, settings = push_trigger(workflow(name))
                self.assertIn('push', trigger)
                self.assertFalse([s for s in settings if s.startswith(('paths', 'branches-ignore', 'tags'))],
                                 f'{name} must not skip main pushes: {settings}')
                branches = [s for s in settings if s.startswith('branches')]
                self.assertTrue(not branches or 'main' in branches[0], branches)


if __name__ == '__main__':
    unittest.main()
