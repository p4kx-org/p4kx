# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Installs a release extension ZIP and runs the synthetic importer checks in native Blender."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import subprocess
from pathlib import Path

REPOSITORY = "user_default"
INSTALLED_ROOT = "P4KX_TEST_INSTALLED_BLENDER_ROOT"


def digest(path: Path) -> str:
    """Returns the SHA-256 of the file."""
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def blender_step(
    blender: Path, args: list[str], root: Path, environment: dict[str, str], name: str
) -> None:
    """Runs one native Blender step and records its output."""
    log = root / f"{name}.log"
    with log.open("xb") as output:
        result = subprocess.run(
            [blender, "--background", "--offline-mode", "--disable-autoexec", *args],
            check=False,
            cwd=root,
            env=environment,
            stdout=output,
            stderr=subprocess.STDOUT,
        )
    if result.returncode:
        raise RuntimeError(f"Blender {name} exited {result.returncode}. See {log}")


def isolated_environment(root: Path) -> dict[str, str]:
    """Gives Blender its own profile, extensions and temporary files."""
    profile = root / "profile"
    scratch = root / "temp"
    profile.mkdir()
    scratch.mkdir()
    environment = {
        key: value
        for key, value in os.environ.items()
        if not key.startswith(("BLENDER_USER_", "BLENDER_SYSTEM_"))
        and key not in ("PYTHONHOME", "PYTHONPATH")
    }
    environment["BLENDER_USER_RESOURCES"] = str(profile)
    environment[INSTALLED_ROOT] = str(profile)
    environment["TMPDIR"] = str(scratch)
    return environment


def main() -> None:
    """Verifies a selected Blender binary and extension on macOS."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--blender", required=True, type=Path)
    parser.add_argument("--zip", required=True, type=Path)
    parser.add_argument("--work-root", required=True, type=Path)
    parser.add_argument("--revision", required=True)
    args = parser.parse_args()
    if platform.system() != "Darwin":
        parser.error("this acceptance requires macOS")
    if len(args.revision) != 40 or any(char not in "0123456789abcdef" for char in args.revision):
        parser.error("--revision requires the full lowercase source commit")
    blender = args.blender.resolve(strict=True)
    extension = args.zip.resolve(strict=True)
    blender_hash, extension_hash = digest(blender), digest(extension)
    root = args.work_root.resolve()
    root.mkdir(parents=True, exist_ok=False)
    environment = isolated_environment(root)
    blender_step(
        blender,
        ["--command", "extension", "install-file", "--repo", REPOSITORY, "--enable", str(extension)],
        root,
        environment,
        "extension-install",
    )
    script = Path(__file__).with_name("blender_smoke.py").resolve()
    blender_step(
        blender,
        ["--python-exit-code", "1", "--python", str(script)],
        root,
        environment,
        "installed-import",
    )
    if (digest(blender), digest(extension)) != (blender_hash, extension_hash):
        raise RuntimeError("Blender or the extension ZIP changed during verification")
    result = {
        "scope": "native-blender-synthetic",
        "revision": args.revision,
        "os_version": platform.mac_ver()[0],
        "cpu": "arm64",
        "blender_sha256": blender_hash,
        "extension_sha256": extension_hash,
        "extension_install": "success",
        "installed_import": "success",
    }
    (root / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
