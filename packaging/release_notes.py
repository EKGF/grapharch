"""Collect breaking commits since the previous release into a leading release-notes section."""
import argparse
from dataclasses import dataclass
from pathlib import Path
import re
import subprocess

MIGRATION_URL = 'https://ekgf.github.io/GraphArch/usage/migration/'
HEADER = re.compile(r'^(?:\* )?[a-z]+(?:\([^()\s]+\))?!: \S')
FOOTER = re.compile(r'^BREAKING[ -]CHANGE: (.+)$')
TRAILER = re.compile(r'^[A-Za-z][A-Za-z-]*: |^[A-Za-z][A-Za-z-]* #')
RELEASE = re.compile(r'v(\d+)\.(\d+)\.(\d+)')
# bounded: one release's commit range, refused beyond this review budget
MAX_COMMITS = 4096


@dataclass(frozen=True)
class BreakingChange:
    """A commit marked breaking by a `!` header or a `BREAKING CHANGE:` footer."""
    subject: str
    details: tuple

    @classmethod
    def parse(cls, message):
        lines = message.strip().splitlines()
        if not lines:
            return None
        subject, details = lines[0].strip(), []
        current = None
        for line in lines[1:]:
            text = line.strip()
            footer = FOOTER.match(text)
            if footer:
                current = [footer.group(1).strip()]
                details.append(current)
            elif current is not None and text and not TRAILER.match(text):
                current.append(text)
            else:
                current = None
                if HEADER.match(text) and not subject.startswith(text.removeprefix('* ')):
                    details.append([text.removeprefix('* ')])
        if not HEADER.match(subject) and not details:
            return None
        return cls(subject, tuple(' '.join(parts) for parts in details))

    def markdown(self):
        return ''.join([f'- {self.subject}\n', *(f'  - {detail}\n' for detail in self.details)])


@dataclass(frozen=True)
class ReleaseNotes:
    """The hand-off body that precedes GitHub's generated notes."""
    changes: tuple

    @classmethod
    def from_messages(cls, messages):
        parsed = (BreakingChange.parse(message) for message in messages)
        return cls(tuple(change for change in parsed if change is not None))

    def markdown(self):
        if not self.changes:
            return ''
        return ''.join(['## Breaking changes\n\n', *(c.markdown() for c in self.changes),
                        f'\nSee the [migration notes]({MIGRATION_URL}) for what to change.\n'])


def previous_tag(version, tags):
    current = tuple(map(int, version.split('.')))
    lower = [tag for tag in tags if RELEASE.fullmatch(tag)
             and tuple(map(int, RELEASE.fullmatch(tag).groups())) < current]
    return max(lower, key=lambda tag: tuple(map(int, RELEASE.fullmatch(tag).groups())), default=None)


def commit_messages(revision, version):
    tags = subprocess.check_output(['git', 'tag', '--list', 'v*'], text=True, encoding='utf-8').split()
    start = previous_tag(version, tags)
    span = f'{start}..{revision}' if start else revision
    output = subprocess.check_output(['git', 'log', '--no-merges', f'--max-count={MAX_COMMITS + 1}',
                                      '--format=%B%x00', span], text=True, encoding='utf-8')
    messages = [message for message in output.split('\0') if message.strip()]
    if len(messages) > MAX_COMMITS:
        raise ValueError('release commit range exceeds bounded review budget')
    return messages


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', required=True)
    parser.add_argument('--revision', default='HEAD')
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    notes = ReleaseNotes.from_messages(commit_messages(args.revision, args.version))
    args.output.write_text(notes.markdown(), encoding='utf-8')


if __name__ == '__main__':
    main()
