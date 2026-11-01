# Publishing Linux binaries

The [release workflow](../../.github/workflows/release.yml) builds Coconut on
native Ubuntu 24.04 runners for Linux x86_64 and ARM64 (aarch64). It compiles all
15 installable executables and the Qt 6 platform-theme plugin, strips and
compresses each executable with UPX, and verifies every compressed binary.
Both architectures must succeed before assets are attached to a GitHub release.
The Qt shared library is included without UPX compression.

## Versions and branches

Use `nightly` for ongoing development. Tags select an exact source revision;
their base version must match `[workspace.package].version` in `Cargo.toml`.

| Channel | Example tag | GitHub release |
| --- | --- | --- |
| Stable | `v0.1.0` | Full release, marked Latest |
| Nightly | `v0.1.0-nightly.20261031.1` | Prerelease, never marked Latest |

Nightly tags use the date in UTC and a sequence number for that day. Increase
the sequence when publishing another build of the same version on the same day.
Nightly builds are explicitly requested through tags or the manual workflow;
pushing the `nightly` branch alone does not publish a release.

Follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html): increase the
patch version for a stable fix (`v0.1.0` → `v0.1.1`). A suffix such as `-rev1`
means a prerelease, so it is unsuitable for a new stable revision. For a nightly
leading up to `v0.1.1`, first set the workspace version to `0.1.1` and use
`v0.1.1-nightly.YYYYMMDD.N`. Keep published tags immutable.

## Publish from a tag

Commit the release workflow and any version change, then push the branch before
the tag. For the current workspace version:

```bash
git push -u origin nightly
git tag -a v0.1.0 -m "Coconut 0.1.0"
git push origin v0.1.0
```

For a nightly of that version:

```bash
tag="v0.1.0-nightly.$(date -u +%Y%m%d).1"
git tag -a "$tag" -m "Coconut $tag"
git push origin "$tag"
```

Pushing a supported tag builds the binaries, creates a draft release, uploads
both archives and `SHA256SUMS`, then publishes the release. Publishing a release
through GitHub also triggers the build for its tag; mark nightly releases as
prereleases. A tag-triggered release needs no manually created GitHub release.

## Publish manually

Once the workflow exists on the repository's default branch, open **Actions →
Publish Linux binaries → Run workflow**, choose the source branch, and enter a
tag. The equivalent CLI command is:

```bash
gh workflow run release.yml --ref nightly -f tag=v0.1.0-nightly.20261031.1
```

An existing tag builds its tagged commit. A new tag builds the selected branch's
commit and is created only after both architectures succeed. The publish job
uses `GITHUB_TOKEN` with `contents: write`; no personal access token is needed
because the sibling repositories are public. An existing release receives the
assets without replacing its notes. Uploads never overwrite existing assets;
if an upload partially fails, remove the incomplete assets before retrying.
Runs for the same tag are serialized.

## Downloads and installation

Release assets have stable, version-independent names:

```text
coconut-linux-x86_64.tar.gz
coconut-linux-aarch64.tar.gz
SHA256SUMS
```

The tag and `release-info.json` inside each archive identify the version, source
commit, architecture, dependency commits, and build-patch checksums. Executables
keep their normal names, including `coconut`, `cfiles`, and `cterm`. Each archive also includes the
installer, Qt plugin, themes, fonts, icons, service definitions, and session
files. Cargo, sources, and sibling repositories are unnecessary for installation.

```bash
sha256sum --check --ignore-missing SHA256SUMS
tar -xzf coconut-linux-x86_64.tar.gz
cd coconut
./scripts/install.sh --force
# Or install for the current user:
./scripts/install.sh --user --force
```

Use the aarch64 archive on ARM64 systems. These are 64-bit GNU/Linux builds,
linked against Ubuntu 24.04 libraries (glibc 2.39); 32-bit x86 and ARM are not
built. Target systems need compatible runtime libraries, including PipeWire,
ALSA, D-Bus, Wayland, xkbcommon, udev, EGL/GLES and Qt 6, plus the GTK/portal
session dependencies described in [SHELL.md](SHELL.md). A compatible Wayland
compositor must be installed separately; the archives do not include Blair or
Alphin daemons.

## Maintain the builder

The workflow checks out CreamUI, Blair, Alphin, and delib beside Coconut at pinned
commits. Update their `ref` values deliberately when changing sibling APIs and
commit the resulting `Cargo.lock` changes. `scripts/build.sh --locked` prevents
the release build from silently resolving a different dependency graph. The
CreamUI pin includes widget file-drop targets and native MIME drag payloads,
which the shell and pinned-app widget require.

The pinned delib commit does not yet expose `MimeApps::applications_for`, which
Coconut's file menus already require. The workflow applies
[delib-mimeapps.patch](../../.github/patches/delib-mimeapps.patch) and runs delib's
tests before compiling Coconut. The patch captures the existing local delib
implementation, including its regression test. Its SHA-256 is recorded in
`release-info.json`. Once delib publishes that API, update the pin and remove
the patch, its application step, and its checksum assertion in the packaging test.

UPX 5.0.2 downloads have architecture-specific SHA-256 checksums in the matrix;
update the version and checksums together.

Local validation:

```bash
python3 scripts/release.py metadata v0.1.0
./scripts/build.sh --locked --no-upx
python3 scripts/release.py package v0.1.0 x86_64
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tests -p test_release.py -v
```

Packaging requires `strip`, UPX, Python 3.11 or newer, and the sibling Git
checkouts. Use a clean staging directory containing the intended executables
and Qt plugin. For a clean delib checkout at the pinned revision, apply the
workflow's delib patch before building. The tests exercise tag rejection, ELF
architecture validation,
compression failure, checksums, compressed execution, and installation from an
extracted archive. The archive/install test requires UPX and `strip` and runs
as an ordinary user.
