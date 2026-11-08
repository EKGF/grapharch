"""Run the precompiled real-store harness, with no second Cargo build."""
import argparse
import os
from pathlib import Path
import subprocess
import time
import urllib.error
import urllib.request

import ci_candidate

IMAGE = 'eclipse/rdf4j-workbench:6.1.0-tomcat@sha256:7a18cecfc879f3143b5a57fe31176a0774fa57e5313f639c6f51756f390f7670'
TEST = 'native_rdf4j_family_publishes_from_restored_pinned_kg'
ENDPOINT = 'http://127.0.0.1:8080/rdf4j-server/repositories/grapharch-ci'


def ready():
    for _ in range(90):
        try:
            with urllib.request.urlopen('http://127.0.0.1:8080/rdf4j-server/protocol', timeout=2) as response:
                if response.status == 200:
                    return
        except (OSError, urllib.error.URLError):
            pass  # Readiness polling is bounded; it never substitutes a different store.
        time.sleep(2)
    raise ValueError('RDF4J did not become ready in the bounded startup window')


def run(root, revision, manifest, fingerprint):
    ci_candidate.verify(root, revision, manifest, 'native-store', fingerprint)
    container = subprocess.check_output(['docker', 'run', '--detach', '--publish', '127.0.0.1:8080:8080', IMAGE], text=True, timeout=180).strip()
    failure = None
    try:
        ready()
        request = urllib.request.Request(ENDPOINT, data=(root / 'tests/fixtures/sparql/native-store.ttl').read_bytes(),
                                         headers={'Content-Type': 'text/turtle'}, method='PUT')
        with urllib.request.urlopen(request, timeout=30) as response:
            if not 200 <= response.status < 300:
                raise ValueError('cannot create the isolated NativeStore repository')
        ci_candidate.run_tests(root, revision, manifest, 'native', False, fingerprint,
                               environment=dict(os.environ, GRAPHARCH_TEST_RDF4J_URL=ENDPOINT))
    except Exception as error:
        failure = error
        raise
    finally:
        try:
            subprocess.run(['docker', 'rm', '--force', container], check=True, timeout=30)
        except Exception as cleanup:
            if failure is None:
                raise
            failure.add_note(f'Container cleanup also failed: {cleanup}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--revision', required=True)
    parser.add_argument('--fingerprint', required=True)
    parser.add_argument('--manifest', type=Path, default=Path('output/ci-engine.json'))
    args = parser.parse_args()
    run(Path(__file__).resolve().parents[1], args.revision, args.manifest, args.fingerprint)
