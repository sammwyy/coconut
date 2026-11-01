#!/usr/bin/env python3
"""Check release versioning, architecture validation, and installable archives."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile
import tomllib
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("release", ROOT / "scripts/release.py")
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)
CURRENT_TAG = "v" + tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.fixture = tempfile.TemporaryDirectory(prefix="coconut release ")
        self.addCleanup(self.fixture.cleanup)
        self.root = Path(self.fixture.name)
        (self.root / "Cargo.toml").write_text('[workspace.package]\nversion = "0.1.0"\n')

    def test_stable_and_nightly_channels(self):
        for tag, channel in (("v0.1.0", "stable"),
                             ("v0.1.0-nightly.20261031.1", "nightly")):
            self.assertEqual(release.release_metadata(tag, self.root),
                             {"tag": tag, "channel": channel})

    def test_invalid_or_mismatched_versions_are_rejected(self):
        for tag in ("0.1.0", "v0.1.0-rev1", "v0.1.0+build.1", "v0.2.0",
                    "v00.1.0", "v0.1.0-nightly.20260230.1",
                    "v0.1.0-nightly.20261031.01", "v0.1.0\nchannel=stable"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.release_metadata(tag, self.root)

    def test_elf_architecture_is_checked(self):
        binary = self.root / "binary"
        for arch, machine in release.MACHINES.items():
            header = bytearray(20)
            header[:6] = b"\x7fELF\x02\x01"
            struct.pack_into("<H", header, 18, machine)
            binary.write_bytes(header)
            release.check_elf(binary, arch)
            other = "aarch64" if arch == "x86_64" else "x86_64"
            with self.assertRaises(ValueError):
                release.check_elf(binary, other)
        binary.write_text("not an executable")
        with self.assertRaises(ValueError):
            release.check_elf(binary, "x86_64")

    def test_empty_staging_is_rejected(self):
        build = self.root / "empty"
        build.mkdir()
        with self.assertRaisesRegex(ValueError, "No staged binaries"):
            release.package_release(ROOT, build, self.root / "dist", "x86_64", CURRENT_TAG)

    @unittest.skipUnless(shutil.which("upx") and shutil.which("strip"), "UPX and strip required")
    def test_archive_can_install_without_sources_or_toolchain(self):
        arch = platform.machine()
        if arch not in release.MACHINES:
            self.skipTest("Requires an x86_64 or aarch64 host")
        build = self.root / "build"
        build.mkdir()
        installer = (ROOT / "scripts/install.sh").read_text()
        binaries = re.search(r"BINARIES=\((.*?)\)", installer, re.S).group(1).split()
        for name in binaries:
            shutil.copy2(shutil.which("true"), build / name)
        plugins = build / "qt/plugins/platformthemes"
        plugins.mkdir(parents=True)
        shutil.copy2(shutil.which("true"), plugins / "libcoconut-qt6.so")
        (build / "qt/build-qt6").mkdir()
        (build / "qt/build-qt6/private.o").write_text("build intermediate")
        output = self.root / "dist"
        with mock.patch.object(release.subprocess, "check_output", return_value="a" * 40):
            release.package_release(ROOT, build, output, arch, CURRENT_TAG)
        archive = output / f"coconut-linux-{arch}.tar.gz"
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        self.assertEqual((output / f"{archive.name}.sha256").read_text(),
                         f"{digest}  {archive.name}\n")
        extracted = self.root / "extracted"
        with tarfile.open(archive) as package:
            self.assertFalse(any("build-qt6" in name for name in package.getnames()))
            package.extractall(extracted, filter="data")
        bundle = extracted / "coconut"
        metadata = json.loads((bundle / "release-info.json").read_text())
        self.assertEqual(metadata["tag"], CURRENT_TAG)
        patch = ROOT / ".github/patches/delib-mimeapps.patch"
        self.assertEqual(metadata["patches"][patch.name], hashlib.sha256(patch.read_bytes()).hexdigest())
        for name in binaries:
            subprocess.run([str(bundle / ".build" / name)], check=True)
        commands = self.root / "commands"
        commands.mkdir()
        for command in ("systemctl", "fc-cache", "update-desktop-database"):
            stub = commands / command
            stub.write_text("#!/bin/sh\nexit 0\n")
            stub.chmod(0o755)
        prefix = self.root / "prefix"
        env = dict(os.environ, PATH=f"{commands}:{os.environ['PATH']}",
                   HOME=str(prefix), XDG_CONFIG_HOME=str(prefix / "config"),
                   XDG_DATA_HOME=str(prefix / "share"),
                   COCONUT_BUILD_DIR=str(bundle / ".build"),
                   COCONUT_INSTALL_BIN=str(prefix / "bin"))
        result = subprocess.run(
            ["bash", str(bundle / "scripts/install.sh"), "--user", "--force"],
            env=env, capture_output=True, text=True
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        for name in binaries + ["startcoconut", "coconut-session"]:
            self.assertTrue(os.access(prefix / "bin" / name, os.X_OK), name)
        self.assertTrue((prefix / "share/coconut/qt/plugins/platformthemes/libcoconut-qt6.so").is_file())

    @unittest.skipUnless(shutil.which("true"), "Native ELF fixture required")
    def test_compression_failure_prevents_archive(self):
        arch = platform.machine()
        if arch not in release.MACHINES:
            self.skipTest("Requires an x86_64 or aarch64 host")
        build = self.root / "build"
        build.mkdir()
        shutil.copy2(shutil.which("true"), build / "coconut")
        plugins = build / "qt/plugins/platformthemes"
        plugins.mkdir(parents=True)
        shutil.copy2(shutil.which("true"), plugins / "libcoconut-qt6.so")
        output = self.root / "dist"
        for successful_steps in (1, 2):
            with self.subTest(successful_steps=successful_steps):
                results = [subprocess.CompletedProcess([], 0)] * successful_steps
                results.append(subprocess.CalledProcessError(1, "upx"))
                with mock.patch.object(release.subprocess, "check_output", return_value="a" * 40), \
                     mock.patch.object(release.subprocess, "run", side_effect=results):
                    with self.assertRaises(subprocess.CalledProcessError):
                        release.package_release(ROOT, build, output, arch, CURRENT_TAG)
                self.assertFalse(list(output.glob("*.tar.gz")))


if __name__ == "__main__":
    unittest.main()
