"""Packaging scripts decode and encode text as UTF-8 on every platform.

Windows runners default to the cp1252 locale codec, which cannot decode the
non-ASCII ontology and metadata files, and a script that writes without an
explicit encoding produces locale-dependent bytes. The 0.2.27 and 0.2.28
releases were tagged but never published because the Windows distribution
job crashed on ``path.read_text()`` in ``distribute.py``.
"""
import ast
from pathlib import Path
import unittest

PACKAGING = Path(__file__).parents[1]
BINARY_MODES = {'rb', 'wb', 'ab', 'xb', 'r+b', 'w+b'}
NON_FILE_RECEIVERS = ('archive', 'zipped', 'tar', 'zip')


class TextCall:
    """A call that reads or writes a file in text mode."""

    def __init__(self, path: Path, node: ast.Call) -> None:
        self.path = path
        self.node = node

    @classmethod
    def all_in(cls, path: Path) -> list['TextCall']:
        tree = ast.parse(path.read_text(encoding='utf-8'))
        calls = (cls(path, node) for node in ast.walk(tree) if isinstance(node, ast.Call))
        return [call for call in calls if call.is_text_io()]

    def is_text_io(self) -> bool:
        function = self.node.func
        if isinstance(function, ast.Attribute) and function.attr in ('read_text', 'write_text'):
            return True
        if isinstance(function, ast.Attribute) and function.attr == 'open':
            receiver = ast.unparse(function.value)
            return not any(part in receiver for part in NON_FILE_RECEIVERS) and self._text_mode(0)
        if isinstance(function, ast.Name) and function.id == 'open':
            return self._text_mode(1)
        return False

    def declares_encoding(self) -> bool:
        return any(keyword.arg == 'encoding' for keyword in self.node.keywords)

    def location(self) -> str:
        return f'{self.path.name}:{self.node.lineno}'

    def _text_mode(self, position: int) -> bool:
        mode = next((k.value for k in self.node.keywords if k.arg == 'mode'), None)
        if mode is None and len(self.node.args) > position:
            mode = self.node.args[position]
        return not (isinstance(mode, ast.Constant) and mode.value in BINARY_MODES)


class TextEncoding(unittest.TestCase):
    def test_every_text_file_access_declares_its_encoding(self):
        scripts = sorted(PACKAGING.glob('*.py'))
        self.assertGreater(len(scripts), 10)
        missing = [
            call.location()
            for script in scripts
            for call in TextCall.all_in(script)
            if not call.declares_encoding()
        ]
        self.assertEqual([], missing, 'text file access without encoding="utf-8"')


if __name__ == '__main__':
    unittest.main()
