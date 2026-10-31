#!/usr/bin/env python3
"""Check dependency selection without changing system packages."""

import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SETUP = ROOT / "scripts/setup.sh"


class SetupTests(unittest.TestCase):
    def setUp(self):
        self.fixture = tempfile.TemporaryDirectory(prefix="coconut setup ")
        self.addCleanup(self.fixture.cleanup)
        self.release = Path(self.fixture.name) / "os-release"
        self.env = dict(os.environ, COCONUT_SETUP_OS_RELEASE=str(self.release))

    def run_setup(self, distribution, *args, like=""):
        self.release.write_text(f'ID={distribution}\nID_LIKE="{like}"\n')
        return subprocess.run([str(SETUP), "--dry-run", *args], env=self.env,
                              capture_output=True, text=True, timeout=10)

    def test_native_package_sets(self):
        packages = {
            "fedora": ("dnf install", "pipewire-devel", "libseat-devel", "qt6-qtbase-private-devel"),
            "ubuntu": ("apt-get update", "libpipewire-0.3-dev", "libseat-dev", "qt6-base-private-dev"),
            "debian": ("apt-get install", "python3-gi", "xdg-desktop-portal"),
            "arch": ("pacman -Syu --needed", "libseat", "qt6-base"),
            "opensuse-tumbleweed": ("zypper install -t pattern devel_basis",
                                   "pipewire-devel", "qt6-base-private-devel"),
        }
        for distribution, expected in packages.items():
            with self.subTest(distribution=distribution):
                result = self.run_setup(distribution)
                self.assertEqual(result.returncode, 0, result.stderr)
                for package in expected:
                    self.assertIn(package, result.stdout)
                for command in result.stdout.splitlines():
                    if command.startswith("  "):
                        self.assertIsNone(re.search(r"\s(?:rust|rustc|cargo|rustup)\s", command))

    def test_compatible_distribution_uses_id_like(self):
        result = self.run_setup("linuxmint", like="ubuntu debian")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("apt-get install", result.stdout)

    def test_unknown_distribution_and_rhel_do_not_install(self):
        for distribution, like in (("unknown", ""), ("rocky", "rhel fedora")):
            with self.subTest(distribution=distribution):
                result = self.run_setup(distribution, like=like)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("install", result.stdout)

    def test_invalid_option_fails_before_reading_release(self):
        result = subprocess.run([str(SETUP), "--unsupported"], env=self.env,
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 2)
        self.assertIn("unknown argument", result.stderr)

    def test_missing_user_toolchain_is_reported_without_installing_it(self):
        commands = Path(self.fixture.name) / "commands"
        commands.mkdir()
        for executable in ("bash", "id"):
            (commands / executable).symlink_to(Path("/usr/bin") / executable)
        self.env["PATH"] = str(commands)
        result = self.run_setup("fedora")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("install rustup and stable as your regular user", result.stderr)
        self.assertNotIn(" rust ", result.stdout)
        self.assertNotIn(" cargo ", result.stdout)


if __name__ == "__main__":
    unittest.main()
