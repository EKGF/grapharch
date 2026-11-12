"""Select the one GraphArch engine an Action invocation installs.

Without selector inputs, the Action installs the release built from its own
source: the version from its own `Cargo.toml` and its own exact commit, which
the release manifest must name as its source. Explicit inputs select either
another verified release (`version` with `expected-revision`) or a source build
of the Action checkout (`source-revision`).
"""
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib

from source_action import validate_checkout

REPOSITORY = 'ekgf/grapharch'


def exact_commit(value, name):
    if not re.fullmatch(r'[a-f0-9]{40}', value) or value == '0' * 40:
        raise ValueError(f'{name} must be an exact nonzero lowercase Git SHA')
    return value


def exact_version(value, name):
    if not re.fullmatch(r'\d+\.\d+\.\d+', value):
        raise ValueError(f'{name} must be an exact MAJOR.MINOR.PATCH release without v')
    return value


class ActionCheckout:
    """The GraphArch source tree this Action invocation executes from.

    A repository Action (`uses: EKGF/grapharch@<ref>`) is identified by the ref
    the runner downloaded. A local Action (`uses: ./path`) has no ref; it is
    identified by the commit of its own clean Git checkout.
    """

    def __init__(self, path, repository, reference):
        self.path = Path(path)
        self.repository = repository
        self.reference = reference

    @classmethod
    def from_environment(cls):
        return cls(os.environ['GITHUB_ACTION_PATH'],
                   os.environ.get('GRAPHARCH_ACTION_REPOSITORY', ''),
                   os.environ.get('GRAPHARCH_ACTION_REF', ''))

    def version(self):
        manifest = tomllib.loads((self.path / 'Cargo.toml').read_text())
        return exact_version(manifest['package']['version'], 'the action checkout Cargo.toml version')

    def commit(self):
        if self.repository:
            return self.referenced_commit()
        if self.reference:
            raise ValueError('the runner reported an action ref without its repository')
        return self.checked_out_commit()

    def referenced_commit(self):
        if self.repository.lower() != REPOSITORY:
            raise ValueError(
                f'the action runs as {self.repository}, not EKGF/grapharch; '
                'a wrapping composite action must supply version and expected-revision')
        if (not re.fullmatch(r'[a-f0-9]{40}', self.reference)
                or self.reference == '0' * 40):
            raise ValueError(
                f'action reference {self.reference!r} is not an exact release commit SHA; '
                'use EKGF/grapharch@<release-commit-sha> # vX.Y.Z, '
                'or supply version and expected-revision')
        return self.reference

    def checked_out_commit(self):
        top = subprocess.run(['git', '-C', str(self.path), 'rev-parse', '--show-toplevel'],
                             text=True, capture_output=True, check=False)
        if top.returncode != 0 or Path(top.stdout.strip()).resolve() != self.path.resolve():
            raise ValueError(f'local action {self.path} is not the root of a Git checkout; '
                             'check out EKGF/grapharch at an exact release commit')
        head = subprocess.check_output(['git', '-C', str(self.path), 'rev-parse', 'HEAD'],
                                       text=True).strip()
        self.require_commit(head)
        return head

    def require_commit(self, revision):
        validate_checkout(self.path, revision)


class EngineSelection:
    """One release to install, or one source checkout to build."""

    RELEASE = 'release'
    SOURCE = 'source'

    def __init__(self, kind, version, revision):
        self.kind = kind
        self.version = version
        self.revision = revision

    @classmethod
    def resolve(cls, version, expected_revision, source_revision, checkout):
        if source_revision:
            if version or expected_revision:
                raise ValueError('source-revision builds the action checkout; '
                                 'omit version and expected-revision')
            revision = exact_commit(source_revision, 'source-revision')
            checkout.require_commit(revision)
            return cls(cls.SOURCE, checkout.version(), revision)
        if version or expected_revision:
            if not (version and expected_revision):
                raise ValueError('supply version and expected-revision together, or omit both '
                                 'to install the release of the action commit')
            return cls(cls.RELEASE, exact_version(version, 'version'),
                       exact_commit(expected_revision, 'expected-revision'))
        return cls(cls.RELEASE, checkout.version(), checkout.commit())

    def outputs(self):
        return f'kind={self.kind}\nversion={self.version}\nrevision={self.revision}\n'


def main():
    chosen = EngineSelection.resolve(os.environ.get('GRAPHARCH_VERSION', ''),
                                     os.environ.get('GRAPHARCH_EXPECTED_REVISION', ''),
                                     os.environ.get('GRAPHARCH_SOURCE_REVISION', ''),
                                     ActionCheckout.from_environment())
    with open(os.environ['GITHUB_OUTPUT'], 'a') as output:
        output.write(chosen.outputs())


if __name__ == '__main__':
    try:
        main()
    except (OSError, KeyError, ValueError, tomllib.TOMLDecodeError,
            subprocess.CalledProcessError) as error:
        print(f'GraphArch selection: {error}', file=sys.stderr)
        sys.exit(1)
