"""npm distinguishes local archive paths from repository shorthand."""
import json
import os
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).parents[1]))

ROOT = Path(__file__).resolve().parents[2]


class NpmPaths(unittest.TestCase):
    def test_platform_and_main_archives_are_absolute_even_for_relative_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            previous = Path.cwd()
            self.addCleanup(os.chdir, previous)
            os.chdir(temporary)
            output = Path('dist')
            output.mkdir()
            (output / 'grapharch-test.json').write_text(json.dumps({
                'npm_package': 'grapharch-test', 'version': '0.2.5',
                'npm_archive': 'grapharch-test-0.2.5.tgz'}))
            (output / 'grapharch-npm.json').write_text(json.dumps({
                'version': '0.2.5', 'archive': 'grapharch-0.2.5.tgz'}))
            with patch('distribute.PLATFORMS', [{'target': 'test'}]), patch('distribute.verify_release'), patch('distribute.npm_command', return_value=['npm']), patch('registry_state.npm_published', return_value=False), patch('subprocess.run') as execute, patch.object(sys, 'argv', ['publish_npm.py', '--output', 'dist']):
                runpy.run_path(str(ROOT / 'packaging/publish_npm.py'), run_name='__main__')
            archives = [Path(call.args[0][-1]) for call in execute.call_args_list]
            self.assertEqual(len(archives), 2)
            self.assertTrue(all(path.is_absolute() for path in archives))
            self.assertEqual({path.parent for path in archives}, {output.resolve()})
