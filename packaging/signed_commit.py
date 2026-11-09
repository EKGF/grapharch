"""Create a verified GitHub-signed metadata commit without changing main."""
import base64
import json
from pathlib import Path
import re
import subprocess
import uuid


def git(*arguments):
    return subprocess.check_output(['git', *arguments], text=True).strip()


def api(path, payload=None):
    arguments = ['gh', 'api', path]
    if payload is not None:
        arguments.extend(['--input', '-'])
    return json.loads(subprocess.check_output(arguments, input=json.dumps(payload) if payload is not None else None, text=True))


def validate_commit(record, source, tree):
    if (record.get('parents') != [{'sha': source}] and
            [parent.get('sha') for parent in record.get('parents', [])] != [source]):
        raise ValueError('signed version commit has an unexpected parent')
    if record['commit']['tree']['sha'] != tree:
        raise ValueError('signed version commit differs from prepared metadata tree')
    if record['commit']['verification']['verified'] is not True:
        raise ValueError('GitHub did not verify the version commit signature')
    if not re.fullmatch('[a-f0-9]{40}', record['sha']):
        raise ValueError('invalid signed commit identity')
    return record['sha']


def create(repository, source, files, headline, body):
    # bounded: exactly the four reviewed version metadata files supplied by caller.
    subprocess.run(['git', 'add', '--', *files], check=True)
    tree = git('write-tree')
    branch = f'grapharch-release/{uuid.uuid4().hex}'
    api(f'repos/{repository}/git/refs', {'ref': f'refs/heads/{branch}', 'sha': source})
    try:
        mutation = '''mutation($input:CreateCommitOnBranchInput!){createCommitOnBranch(input:$input){commit{oid}}}'''
        payload = {'query': mutation, 'variables': {'input': {
            'branch': {'repositoryNameWithOwner': repository, 'branchName': branch},
            'expectedHeadOid': source,
            'message': {'headline': headline, 'body': body},
            'fileChanges': {'additions': [
                {'path': name, 'contents': base64.b64encode(Path(name).read_bytes()).decode('ascii')}
                for name in files]}}}}
        result = api('graphql', payload)
        if result.get('errors'):
            raise ValueError(f'GitHub signed commit failed: {result["errors"]}')
        revision = result['data']['createCommitOnBranch']['commit']['oid']
        record = api(f'repos/{repository}/commits/{revision}')
        validate_commit(record, source, tree)
        subprocess.run(['git', 'fetch', 'origin', f'refs/heads/{branch}'], check=True)
        if git('rev-parse', 'FETCH_HEAD') != revision:
            raise ValueError('signed staging branch changed unexpectedly')
        # Metadata already matches this exact verified tree; no source changes discarded.
        subprocess.run(['git', 'checkout', '--detach', revision], check=True)
        return revision
    finally:
        # Delete only this invocation's temporary ref. Never alter source/main on failure.
        subprocess.run(['gh', 'api', '--method', 'DELETE', f'repos/{repository}/git/refs/heads/{branch}'], check=True)
