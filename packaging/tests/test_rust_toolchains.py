"""The Rust cache key must depend only on the pinned compiler, not the runner image."""
import re
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parents[1]))
from rust_toolchains import pinned_channel, unpinned

ROOT = Path(__file__).resolve().parents[2]


class RustToolchains(unittest.TestCase):
    def test_runner_image_toolchains_are_removed_but_the_pin_is_kept(self):
        # Observed on two macos-15 arm64 images: the preinstalled stable moved
        # from 1.99.1 to 1.100.0, changing the env hash e3ca4ace -> f4fc3ace.
        channel = pinned_channel()
        listing = (f"stable-aarch64-apple-darwin (default)\n"
                   f"1.99.1-aarch64-apple-darwin\n"
                   f"{channel}-aarch64-apple-darwin (active)\n")
        self.assertEqual(unpinned(listing, channel),
                         ["stable-aarch64-apple-darwin", "1.99.1-aarch64-apple-darwin"])
        self.assertEqual(unpinned(f"{channel}-x86_64-pc-windows-msvc (active, default)\n", channel), [])
        self.assertEqual(unpinned("no installed toolchains\n", channel), [])

    def test_a_different_dated_nightly_is_not_mistaken_for_the_pin(self):
        self.assertEqual(unpinned("nightly-2026-10-300-aarch64-apple-darwin\n"
                                  "nightly-aarch64-apple-darwin\n", "nightly-2026-10-30"),
                         ["nightly-2026-10-300-aarch64-apple-darwin", "nightly-aarch64-apple-darwin"])

    def test_floating_channels_cannot_be_the_pin(self):
        for channel in ("stable", "nightly", "beta"):
            with self.subTest(channel=channel), self.assertRaises(ValueError):
                import tempfile
                with tempfile.TemporaryDirectory() as directory:
                    (Path(directory) / "rust-toolchain.toml").write_text(
                        f'[toolchain]\nchannel = "{channel}"\n')
                    pinned_channel(Path(directory))

    def test_every_cached_release_toolchain_setup_first_removes_runner_toolchains(self):
        workflow = (ROOT / '.github/workflows/distribute.yml').read_text(encoding='utf-8')
        setups = [match.start() for match in
                  re.finditer(r'- uses: actions-rust-lang/setup-rust-toolchain@', workflow)]
        self.assertTrue(setups)
        for start in setups:
            previous = workflow[:start].rsplit('\n      - ', 2)[-1]
            self.assertIn('packaging/rust_toolchains.py', previous)


if __name__ == '__main__':
    unittest.main()
