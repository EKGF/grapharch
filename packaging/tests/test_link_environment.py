"""Multiple C++ libraries in one crate must share a valid static runtime policy."""

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from link_environment import environment


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


if __name__ == "__main__":
    unittest.main()
