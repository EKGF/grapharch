import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "check_release_dependencies",
    Path(__file__).parents[1] / "check_release_dependencies.py",
)
dependencies = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dependencies)


class ReleaseDependenciesTest(unittest.TestCase):
    def test_native_tls_is_rejected_but_system_ca_discovery_is_allowed(self):
        dependencies.validate_tree(
            "grapharch v0.0.1\nrustls v0.23.25\nopenssl-probe v0.1.6\n", "linux"
        )
        for name in ["openssl-sys", "openssl", "native-tls", "hyper-tls"]:
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, name):
                dependencies.validate_tree(
                    f"grapharch v0.0.1\n{name} v1.0.0 (*)\n", "linux"
                )

    def test_unused_legacy_pdf_engine_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unused legacy PDF"):
            dependencies.validate_tree("grapharch v0.0.1\nprintpdf v0.8.2\n", "linux")
