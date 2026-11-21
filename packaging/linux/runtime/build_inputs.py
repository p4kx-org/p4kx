"""Binds the retained upstream dependency builds to the actual native static linker inputs."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shlex
from urllib.parse import urlparse


BUILT_ARCHIVES = {
    "libfuse": (("build/lib/libfuse3.a", "/usr/lib/libfuse3.a"),),
    "squashfuse": (
        (".libs/libsquashfuse.a", "/usr/local/lib/libsquashfuse.a"),
        (".libs/libsquashfuse_ll.a", "/usr/local/lib/libsquashfuse_ll.a"),
    ),
}


def digest(path):
    """Returns a file's SHA-256 digest without loading it into memory."""
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def inside(root, relative):
    """Resolves a retained build input without following a path outside its owned root."""
    path = (root / relative).resolve(strict=True)
    if not path.is_relative_to(root.resolve(strict=True)):
        raise ValueError(f"dependency build input escapes its root: {relative}")
    return path


def source_inputs(inputs, root):
    """Validates the exact archives that the verified upstream recipe used to build libraries."""
    found = {}
    for dependency in inputs["source_dependencies"]:
        name = dependency["name"]
        if name not in BUILT_ARCHIVES or name in found:
            raise ValueError(f"unexpected built dependency: {name}")
        filename = urlparse(dependency["url"]).path.rsplit("/", 1)[-1]
        archive = inside(root, filename)
        if digest(archive) != dependency["sha256"]:
            raise ValueError(f"compiled dependency source digest mismatch: {name}")
        directory = dependency["archive_root"]
        if not re.fullmatch(r"[A-Za-z0-9_.+-]+", directory):
            raise ValueError(f"invalid dependency source root: {name}")
        source = inside(root, directory)
        if not source.is_dir():
            raise ValueError(f"compiled dependency source tree is absent: {name}")
        found[name] = {**dependency, "archive": filename, "directory": directory}
    if set(found) != set(BUILT_ARCHIVES):
        raise ValueError("the native build dependency inventory is incomplete")
    return found


def prepare(upstream, inputs, root):
    """Keeps the pinned upstream recipe and its verified source/build workspace available."""
    root = root.resolve()
    root.mkdir()
    script = upstream / "scripts/common/install-dependencies.sh"
    content = script.read_text()
    original = digest(script)
    for dependency in inputs["source_dependencies"]:
        if dependency["url"] not in content or dependency["sha256"] not in content:
            raise ValueError("upstream recipe and locked source URL/digest differ")
    marker = 'tmpdir="$(mktemp -d)"'
    if content.count(marker) != 1:
        raise ValueError("upstream recipe workspace declaration changed")
    content = content.replace(marker, f"tmpdir={shlex.quote(str(root))}")
    cleanup = r'\n# clean it up whenever the script exits\ncleanup\(\) \{\n.*?\n\}\ntrap cleanup EXIT\n'
    content, count = re.subn(cleanup, "\n", content, flags=re.DOTALL)
    if count != 1:
        raise ValueError("upstream recipe cleanup contract changed")
    (root / "upstream-install-dependencies.sh").write_bytes(script.read_bytes())
    script.write_bytes(content.encode("utf-8"))
    receipt = {"original_recipe_sha256": original, "retained_recipe_sha256": digest(script), "retained_recipe_path": str(script.resolve())}
    (root / "recipe.json").write_text(json.dumps(receipt, indent=2) + "\n")


def record(inputs, root, layouts=BUILT_ARCHIVES):
    """Records exact compiled and installed archives before the runtime linker executes."""
    sources = source_inputs(inputs, root)
    if set(layouts) != set(BUILT_ARCHIVES):
        raise ValueError("the compiled archive layout inventory is incomplete")
    archives = []
    for name, paths in layouts.items():
        source = sources[name]
        for built, installed in paths:
            relative = f"{source['directory']}/{built}"
            build_archive = inside(root, relative)
            installed_archive = Path(installed).resolve(strict=True)
            sha256 = digest(build_archive)
            if digest(installed_archive) != sha256:
                raise ValueError(f"installed archive differs from its actual build output: {installed}")
            archives.append({"source": name, "build_archive": relative, "installed_path": str(installed_archive), "sha256": sha256})
    receipt = json.loads((root / "recipe.json").read_text())
    validate_recipe(root, receipt)
    manifest = {"sources": sources, "archives": archives, "recipe": receipt}
    (root / "build-inputs.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def validate(inputs, root):
    """Rechecks retained source and native archive identities after the runtime link."""
    manifest = json.loads((root / "build-inputs.json").read_text())
    if source_inputs(inputs, root) != manifest["sources"]:
        raise ValueError("retained source inputs differ from the actual build receipt")
    validate_recipe(root, manifest["recipe"])
    expected = {(name, f"{manifest['sources'][name]['directory']}/{relative}") for name, paths in BUILT_ARCHIVES.items() for relative, _ in paths}
    actual = {(archive["source"], archive["build_archive"]) for archive in manifest["archives"]}
    if actual != expected or len(manifest["archives"]) != len(expected):
        raise ValueError("the native compiled archive receipt is incomplete or ambiguous")
    for archive in manifest["archives"]:
        if digest(inside(root, archive["build_archive"])) != archive["sha256"]:
            raise ValueError("compiled dependency archive changed after its receipt")
        bound_archive(Path(archive["installed_path"]), manifest)
    return manifest


def validate_recipe(root, receipt):
    """Checks the preserved upstream recipe and the exact recipe used by the native build."""
    if digest(root / "upstream-install-dependencies.sh") != receipt["original_recipe_sha256"]:
        raise ValueError("original upstream build recipe changed")
    if digest(Path(receipt["retained_recipe_path"])) != receipt["retained_recipe_sha256"]:
        raise ValueError("the actual dependency build recipe changed")


def bound_archive(path, manifest):
    """Matches a linked archive by exact resolved installation path and pre-link digest."""
    resolved = path.resolve(strict=True)
    names = {Path(item["installed_path"]).name for item in manifest["archives"]}
    matches = [item for item in manifest["archives"] if str(resolved) == item["installed_path"]]
    if len(matches) == 1:
        archive = matches[0]
        if digest(resolved) != archive["sha256"]:
            raise ValueError(f"linked archive differs from its pre-link build receipt: {path}")
        return archive
    if matches or resolved.name in names:
        raise ValueError(f"linked dependency archive has an unowned path: {path}")
    return None


def main():
    """Prepares retained upstream inputs or records their actual native build outputs."""
    parser = argparse.ArgumentParser()
    parser.add_argument("operation", choices=("prepare", "record"))
    parser.add_argument("--upstream", required=True, type=Path)
    parser.add_argument("--inputs", required=True, type=Path)
    parser.add_argument("--root", required=True, type=Path)
    arguments = parser.parse_args()
    inputs = json.loads(arguments.inputs.read_text())
    if arguments.operation == "prepare":
        prepare(arguments.upstream, inputs, arguments.root)
    else:
        record(inputs, arguments.root)


if __name__ == "__main__":
    main()
