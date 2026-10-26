"""Multiple C++ libraries in one crate must share a valid static runtime policy."""

from pathlib import Path
import json
import os
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from link_environment import environment, linux_runtime_environment, write_shell


class LinkEnvironmentTests(unittest.TestCase):
    def test_runtime_can_be_declared_by_two_independent_cpp_builds(self):
        flags = environment("x86_64-unknown-linux-gnu")
        runtime = flags["CXXSTDLIB"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            # An empty archive isolates rustc's native-library metadata handling
            # without needing the host to supply Linux's C++ runtime.
            for name in ("libstdc++.a", "stdc++.lib"):
                (root / name).write_bytes(b"!<arch>\n")
            source = root / "probe.rs"
            source.write_text("pub fn probe() -> u8 { 1 }\n")
            result = subprocess.run(
                ["rustc", "--crate-type=rlib", "--crate-name=link_probe",
                 str(source), "-L", f"native={root}", "-l", runtime,
                 "-l", runtime, "-o", str(root / "probe.rlib")],
                capture_output=True, text=True, timeout=30, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue((root / "probe.rlib").is_file())

    def test_linux_policy_does_not_override_other_platform_runtimes(self):
        self.assertEqual(environment("aarch64-apple-darwin"), {})
        self.assertEqual(environment("aarch64-pc-windows-msvc"), {})
        with self.assertRaises(ValueError):
            environment("unknown-target")

    def test_compiler_runtime_search_path_survives_spaces_and_shell_transport(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            library = root / "compiler runtime/libstdc++.a"
            library.parent.mkdir()
            library.write_bytes(b"!<arch>\n")
            compiler = root / "compiler.py"
            compiler.write_text(f"print({str(library)!r})\n")
            settings = linux_runtime_environment(
                compiler=(sys.executable, str(compiler)),
                previous={"RUSTFLAGS": "-D warnings"},
            )
            self.assertEqual(settings["CXXSTDLIB"], "static=stdc++")
            self.assertEqual(settings["CARGO_ENCODED_RUSTFLAGS"].split("\x1f"),
                             ["-D", "warnings", "-L", f"native={library.parent}"])
            script = root / "environment.sh"
            write_shell(script, settings)
            result = subprocess.run(
                ["bash", "-c", '. "$1" || exit $?; exec "$2" -c "$3"',
                 "link-probe", str(script), sys.executable,
                 'import json,os; print(json.dumps({key:os.environ[key] for key in '
                 '["CXXSTDLIB", "CARGO_ENCODED_RUSTFLAGS"]}))'],
                capture_output=True, text=True, check=True, timeout=30,
                env=os.environ.copy(),
            )
            self.assertEqual(json.loads(result.stdout), settings)


if __name__ == "__main__":
    unittest.main()
