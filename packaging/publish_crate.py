"""Publish the checked source crate without compiling its already-verified code again."""
from pathlib import Path
import subprocess

from distribute import version
from registry_state import crate_published

subprocess.run(['cargo', 'package', '--locked', '--package', 'grapharch', '--no-verify'], check=True)
archive = Path('target/package') / f'grapharch-{version()}.crate'
if crate_published(version(), archive):
    print('Already published identical crates.io source archive')
else:
    subprocess.run(['cargo', 'publish', '--locked', '--package', 'grapharch', '--no-verify'], check=True)
