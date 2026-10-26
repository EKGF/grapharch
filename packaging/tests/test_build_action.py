import importlib.util
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "build_action", Path(__file__).parents[1] / "build_action.py"
)
build_action = importlib.util.module_from_spec(spec)
spec.loader.exec_module(build_action)


class BuildActionTest(unittest.TestCase):
    def environment(self, root):
        return {
            "GRAPHARCH_MODE": "build-family",
            "GRAPHARCH_BINARY": "/tools with spaces/grapharch",
            "GRAPHARCH_SOURCE": "sources with spaces",
            "GRAPHARCH_SOURCE_KIND": "directory",
            "GRAPHARCH_DEFINITIONS": "project definitions.ttl",
            "GRAPHARCH_PROJECT": "urn:example:project",
            "GRAPHARCH_TARGET": "urn:example:spec",
            "GRAPHARCH_OUTPUT": str(root / "build/my docs"),
            "GITHUB_OUTPUT": str(root / "outputs"),
        }

    def test_build_uses_argument_array_and_structured_report(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "build/my docs"
            publication = output / "site"
            publication.mkdir(parents=True)
            (publication / "index.html").write_text("<html></html>")
            (output / "spec.pdf").write_bytes(b"%PDF")
            (output / "documentation.nq").write_text("")
            report = {
                "schema_version": 1,
                "publication_path": str(publication),
                "pdf_path": str(output / "spec.pdf"),
                "kg_path": str(output / "documentation.nq"),
            }
            result = subprocess.CompletedProcess([], 0, json.dumps(report))
            with (
                patch.dict(os.environ, self.environment(root)),
                patch.object(
                    build_action.subprocess, "run", return_value=result
                ) as run,
            ):
                self.assertEqual(build_action.main(), 0)
            command = run.call_args.args[0]
            self.assertEqual(
                command,
                [
                    "/tools with spaces/grapharch",
                    "publication",
                    "build-family",
                    "--source",
                    "sources with spaces",
                    "--definitions",
                    "project definitions.ttl",
                    "--project",
                    "urn:example:project",
                    "--target",
                    "urn:example:spec",
                    "--output-root",
                    str(output),
                    "--json",
                ],
            )
            self.assertEqual(
                (root / "outputs").read_text(),
                f"publication-path={publication.resolve()}\npdf-path={(output / 'spec.pdf').resolve()}\nkg-path={(output / 'documentation.nq').resolve()}\n",
            )

    def test_exact_failure_status_without_report_or_fallback(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with (
                patch.dict(os.environ, self.environment(root)),
                patch.object(
                    build_action.subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess([], 37, "not json"),
                ) as run,
            ):
                self.assertEqual(build_action.main(), 37)
            self.assertEqual(run.call_count, 1)
            self.assertFalse((root / "outputs").exists())

    def test_rejects_malformed_success_reports(self):
        for report in ["human output", "{}", '{"publication_path":"line\\nbreak"}']:
            with (
                self.subTest(report=report),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                with (
                    patch.dict(os.environ, self.environment(root)),
                    patch.object(
                        build_action.subprocess,
                        "run",
                        return_value=subprocess.CompletedProcess([], 0, report),
                    ),
                    self.assertRaises(ValueError),
                ):
                    build_action.main()
                self.assertFalse((root / "outputs").exists())

    def test_install_mode_does_not_run_another_command(self):
        with (
            patch.dict(os.environ, {"GRAPHARCH_MODE": "install"}),
            patch.object(build_action.subprocess, "run") as run,
        ):
            self.assertEqual(build_action.main(), 0)
            run.assert_not_called()

    def test_missing_inputs_and_unknown_modes_fail_before_execution(self):
        for override in [
            {"GRAPHARCH_MODE": "shell"},
            {"GRAPHARCH_DEFINITIONS": ""},
            {"GRAPHARCH_SOURCE_KIND": "auto"},
        ]:
            with tempfile.TemporaryDirectory() as temporary:
                environment = self.environment(Path(temporary)) | override
                with (
                    patch.dict(os.environ, environment),
                    patch.object(build_action.subprocess, "run") as run,
                    self.assertRaises(ValueError),
                ):
                    build_action.main()
                run.assert_not_called()

    def test_report_paths_must_exist_under_requested_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            report = {
                "schema_version": 1,
                "publication_path": str(root),
                "pdf_path": str(root / "spec.pdf"),
                "kg_path": str(root / "documentation.nq"),
            }
            with (
                patch.dict(os.environ, self.environment(root)),
                patch.object(
                    build_action.subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess([], 0, json.dumps(report)),
                ),
                self.assertRaisesRegex(ValueError, "output root"),
            ):
                build_action.main()
            self.assertFalse((root / "outputs").exists())

    def test_checkout_mode_is_explicit(self):
        with tempfile.TemporaryDirectory() as temporary:
            environment = self.environment(Path(temporary)) | {
                "GRAPHARCH_SOURCE_KIND": "checkout"
            }
            with (
                patch.dict(os.environ, environment),
                patch.object(
                    build_action.subprocess,
                    "run",
                    return_value=subprocess.CompletedProcess([], 37, ""),
                ) as run,
            ):
                self.assertEqual(build_action.main(), 37)
            command = run.call_args.args[0]
            self.assertIn("--local-repository", command)
            self.assertNotIn("--source", command)

    def test_embedded_hints_need_no_external_definitions_or_identifiers(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            environment = self.environment(root) | {
                "GRAPHARCH_DEFINITIONS": "",
                "GRAPHARCH_PROJECT": "",
                "GRAPHARCH_TARGET": "",
            }
            with patch.dict(os.environ, environment):
                self.assertEqual(
                    build_action.configuration(),
                    [
                        "publication", "build-family", "--source",
                        "sources with spaces", "--output-root",
                        str(root / "build/my docs"),
                    ],
                )
