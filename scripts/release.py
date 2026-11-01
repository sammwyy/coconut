#!/usr/bin/env python3
"""Validate release tags and package native Linux release binaries."""

import argparse
from datetime import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
NUMBER = r"(?:0|[1-9][0-9]*)"
TAG = re.compile(
    rf"v(?P<version>{NUMBER}\.{NUMBER}\.{NUMBER})"
    rf"(?P<nightly>-nightly\.(?P<date>[1-9][0-9]{{7}})\.{NUMBER})?"
)
MACHINES = {"x86_64": 62, "aarch64": 183}


def release_metadata(tag, root):
    match = TAG.fullmatch(tag)
    if not match:
        raise ValueError("Use vX.Y.Z or vX.Y.Z-nightly.YYYYMMDD.N")
    workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]
    version = workspace["package"]["version"]
    if match["version"] != version:
        raise ValueError(f"Tag version must match workspace version {version}")
    if match["nightly"]:
        datetime.strptime(match["date"], "%Y%m%d")
    return {"tag": tag, "channel": "nightly" if match["nightly"] else "stable"}


def check_elf(path, arch):
    with path.open("rb") as source:
        header = source.read(20)
    if (
        len(header) != 20
        or header[:6] != b"\x7fELF\x02\x01"
        or struct.unpack_from("<H", header, 18)[0] != MACHINES[arch]
    ):
        raise ValueError(f"Expected a Linux {arch} ELF binary: {path}")


def package_release(root, build, output, arch, tag):
    metadata = release_metadata(tag, root)
    binaries = sorted(path for path in build.iterdir() if path.is_file())
    if not binaries:
        raise ValueError(f"No staged binaries in {build}")
    plugins = sorted((build / "qt/plugins/platformthemes").glob("libcoconut-qt*.so"))
    if not plugins:
        raise ValueError("The staged Qt platform-theme plugin is missing")
    for path in binaries + plugins:
        check_elf(path, arch)
    metadata["architecture"] = arch
    metadata["commit"] = subprocess.check_output(
        ["git", "-C", str(root), "rev-parse", "HEAD"], text=True
    ).strip()
    metadata["dependencies"] = {
        name: subprocess.check_output(
            ["git", "-C", str(root.parent / name), "rev-parse", "HEAD"], text=True
        ).strip()
        for name in ("creamui", "blair", "alphin", "delib")
    }
    metadata["patches"] = {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted((root / ".github/patches").glob("*.patch"))
    }
    output.mkdir(parents=True, exist_ok=True)
    name = f"coconut-linux-{arch}.tar.gz"
    with tempfile.TemporaryDirectory(prefix="coconut-release-") as directory:
        bundle = Path(directory) / "coconut"
        staged = bundle / ".build"
        staged.mkdir(parents=True)
        for binary in binaries:
            destination = staged / binary.name
            shutil.copy2(binary, destination)
            subprocess.run(["strip", "--strip-unneeded", str(destination)], check=True)
            subprocess.run(["upx", "-q", str(destination)], check=True)
            subprocess.run(["upx", "-t", str(destination)], check=True)
        plugin_dir = staged / "qt/plugins/platformthemes"
        plugin_dir.mkdir(parents=True)
        for plugin in plugins:
            shutil.copy2(plugin, plugin_dir / plugin.name)
        for resource in ("assets", "themes", "packaging"):
            shutil.copytree(root / resource, bundle / resource)
        resources = sorted(root.glob("apps/*/data")) + sorted(root.glob("integrations/*/data"))
        for resource in resources:
            shutil.copytree(resource, bundle / resource.relative_to(root))
        (bundle / "scripts").mkdir()
        shutil.copy2(root / "scripts/install.sh", bundle / "scripts/install.sh")
        (bundle / "release-info.json").write_text(json.dumps(metadata, indent=2) + "\n")
        (bundle / "INSTALL.txt").write_text(
            "Install: ./scripts/install.sh --force\n"
            "User install: ./scripts/install.sh --user --force\n"
            "Select a compositor: ./scripts/install.sh blair --force\n\n"
            "Built on Ubuntu 24.04 (glibc 2.39). Native runtime libraries and a\n"
            "Wayland compositor must be installed separately. See:\n"
            "https://github.com/sammwyy/creamshell/blob/nightly/docs/dev/RELEASES.md\n"
        )
        with tarfile.open(output / name, "w:gz") as archive:
            archive.add(bundle, arcname="coconut")
    with (output / name).open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    (output / f"{name}.sha256").write_text(f"{digest}  {name}\n")
    print(output / name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    metadata = commands.add_parser("metadata")
    metadata.add_argument("tag")
    package = commands.add_parser("package")
    package.add_argument("tag")
    package.add_argument("arch", choices=MACHINES)
    package.add_argument(
        "--build", type=Path,
        default=Path(os.environ.get("COCONUT_BUILD_DIR", ROOT / ".build")),
    )
    package.add_argument("--output", type=Path, default=ROOT / ".build/dist")
    args = parser.parse_args()
    try:
        if args.command == "metadata":
            for key, value in release_metadata(args.tag, ROOT).items():
                print(f"{key}={value}")
        else:
            package_release(ROOT, args.build.resolve(), args.output.resolve(), args.arch, args.tag)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"release.py: {error}\n")


if __name__ == "__main__":
    main()
