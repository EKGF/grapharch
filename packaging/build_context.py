"""Bind a generator build number to its actual GitHub producer and source."""
from dataclasses import asdict, dataclass, replace
import argparse
import json
import os
from pathlib import Path
import re

from source_action import validate_checkout
from auto_release import verify_release


@dataclass(frozen=True)
class BuildContext:
    repository: str
    workflow_ref: str
    revision: str
    run_id: int
    run_number: int
    run_attempt: int

    @classmethod
    def parse(cls, value):
        if isinstance(value, str):
            if len(value) > 4096:
                raise ValueError('generator build context exceeds byte budget')
            value = json.loads(value)
        if not isinstance(value, dict) or set(value) != set(cls.__dataclass_fields__):
            raise ValueError('generator build context requires exactly its typed fields')
        result = cls(**value)
        if not isinstance(result.repository, str) or len(result.repository) > 201 or not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+', result.repository) or any(part in ('.', '..') for part in result.repository.split('/')):
            raise ValueError('invalid generator build repository')
        if not isinstance(result.workflow_ref, str) or len(result.workflow_ref) > 1024 or not re.fullmatch(re.escape(result.repository) + r'/.github/workflows/[A-Za-z0-9_.-]+\.ya?ml@refs/[^\x00-\x1f\x7f]+', result.workflow_ref):
            raise ValueError('invalid generator build workflow')
        if not isinstance(result.revision, str) or not re.fullmatch(r'[a-f0-9]{40}', result.revision) or result.revision == '0' * 40:
            raise ValueError('invalid generator build revision')
        if any(type(number) is not int or not 0 < number <= 2**64 - 1 for number in (result.run_id, result.run_number, result.run_attempt)):
            raise ValueError('generator build numbers must be positive integers')
        return result

    @classmethod
    def current(cls, revision):
        if os.environ.get('GITHUB_ACTIONS') != 'true' or os.environ.get('GITHUB_SHA') != revision:
            raise ValueError('generator build context must match the actual GitHub source revision')
        values = {}
        for name in ('run_id', 'run_number', 'run_attempt'):
            value = os.environ.get('GITHUB_' + name.upper(), '')
            if not re.fullmatch(r'[1-9][0-9]{0,19}', value):
                raise ValueError('missing or invalid GitHub generator build number')
            values[name] = int(value)
        result = cls.parse(dict(repository=os.environ.get('GITHUB_REPOSITORY'),
            workflow_ref=os.environ.get('GITHUB_WORKFLOW_REF'), revision=revision, **values))
        return result

    def validate_consumer(self, revision):
        current = self.current(revision)
        if self.revision != revision or self.repository != current.repository or self.workflow_ref != current.workflow_ref or self.run_id != current.run_id or self.run_number != current.run_number or self.run_attempt > current.run_attempt:
            raise ValueError('compiled generator belongs to another build or a future producer attempt')

    def json(self):
        return json.dumps(asdict(self), separators=(',', ':'), sort_keys=True)


def prepared_context(revision):
    value = os.environ.get('GRAPHARCH_BUILD_CONTEXT', '')
    if not value:
        return None
    context = BuildContext.parse(value)
    if context.revision != revision:
        raise ValueError('generator build context differs from pinned source revision')
    return context


def release_context(revision, source, run):
    """Bind a metadata-only release checkout after verifying its actual source proof."""
    verify_release(os.environ['GITHUB_REPOSITORY'], int(run), revision, source)
    actual = BuildContext.current(os.environ['GITHUB_SHA'])
    return replace(actual, revision=revision)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--revision', required=True)
    parser.add_argument('--verified-source', default=os.environ.get('GRAPHARCH_VERIFIED_SOURCE', ''))
    parser.add_argument('--verified-run', default=os.environ.get('GRAPHARCH_VERIFIED_RUN', ''))
    arguments = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    validate_checkout(root, arguments.revision)
    if bool(arguments.verified_source) != bool(arguments.verified_run):
        raise ValueError('release build context requires source and full validation run together')
    context = (release_context(arguments.revision, arguments.verified_source, arguments.verified_run)
               if arguments.verified_run else BuildContext.current(arguments.revision))
    with Path(os.environ['GITHUB_ENV']).open('a') as output:
        output.write('GRAPHARCH_BUILD_CONTEXT=' + context.json() + '\n')


if __name__ == '__main__':
    main()
