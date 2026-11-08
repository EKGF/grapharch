from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

sys.path.insert(0, str(Path(__file__).parents[1]))
import ci_native_store as store


class NativeStoreTests(unittest.TestCase):
    def test_required_ignored_test_uses_precompiled_graph_and_always_cleans_container(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / 'tests/fixtures/sparql/native-store.ttl'
            source.parent.mkdir(parents=True); source.write_text('original fixture')
            response = MagicMock(); response.__enter__.return_value.status = 204
            with patch.object(store.ci_candidate, 'verify'), patch.object(store.subprocess, 'check_output', return_value='a' * 64), patch.object(store, 'ready'), patch.object(store.urllib.request, 'urlopen', return_value=response), patch.object(store.ci_candidate, 'run_tests', side_effect=ValueError('original native failure')) as run, patch.object(store.subprocess, 'run') as cleanup:
                with self.assertRaisesRegex(ValueError, 'original native failure'):
                    store.run(root, 'b' * 40, root / 'manifest', 'c' * 64)
                self.assertEqual(run.call_args.args[3], 'native')
                self.assertEqual(run.call_args.kwargs['environment']['GRAPHARCH_TEST_RDF4J_URL'], store.ENDPOINT)
                self.assertEqual(cleanup.call_args.args[0], ['docker', 'rm', '--force', 'a' * 64])

    def test_startup_is_bounded_and_does_not_substitute_another_endpoint(self):
        with patch.object(store.urllib.request, 'urlopen', side_effect=OSError('not ready')) as request, patch.object(store.time, 'sleep'):
            with self.assertRaisesRegex(ValueError, 'bounded startup'):
                store.ready()
            self.assertEqual(request.call_count, 90)
            self.assertTrue(all(call.args[0] == 'http://127.0.0.1:8080/rdf4j-server/protocol' for call in request.call_args_list))
