"""Collects the corresponding source and original licences of a static AppImage runtime."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import urllib.request


def digest(path):
    """Returns a file's SHA-256 digest without loading it into memory."""
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def download(url, destination, expected=None):
    """Downloads an HTTPS source file and checks its expected digest."""
    if not url.startswith("https://"):
        raise ValueError("source downloads require HTTPS")
    with urllib.request.urlopen(url, timeout=120) as response, destination.open("wb") as output:
        shutil.copyfileobj(response, output)
    actual = digest(destination)
    if expected is not None and actual != expected:
        raise ValueError(f"source digest mismatch for {url}")
    return actual


def installed_packages():
    """Returns Alpine's installed package records."""
    records = []
    for paragraph in Path("/lib/apk/db/installed").read_text().split("\n\n"):
        fields = {}
        for line in paragraph.splitlines():
            if len(line) > 2 and line[1] == ":":
                fields[line[0]] = line[2:]
        if "P" in fields:
            records.append(fields)
    return records


def copy_licences(source, destination):
    """Copies original licence notices from an unpacked source tree."""
    copied = []
    for path in sorted(source.rglob("*")):
        name = path.name.upper()
        if path.is_file() and not path.is_symlink() and name.startswith(("LICENSE", "COPYING", "COPYRIGHT", "NOTICE")):
            relative = path.relative_to(source)
            output = destination / relative
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, output)
            copied.append({"path": str(relative), "sha256": digest(output)})
    if not copied:
        raise ValueError(f"no original licence notices found in {source}")
    return copied


def archive_source(dependency, sources, licences):
    """Collects a pinned upstream source archive and its original notices."""
    name = dependency["name"]
    directory = sources / name
    directory.mkdir()
    archive = directory / "source.tar"
    source_digest = download(dependency["url"], archive, dependency["sha256"])
    unpacked = directory / "unpacked"
    unpacked.mkdir()
    with tarfile.open(archive) as tar:
        tar.extractall(unpacked, filter="data")
    notices = copy_licences(unpacked, licences / name)
    return {**dependency, "source_sha256": source_digest, "licences": notices}


def alpine_source(origin, packages, sources, licences):
    """Collects installed static library sources using their signed package recipe commits."""
    installed = [entry for entry in packages if entry.get("o") == origin]
    identities = {(entry.get("V"), entry.get("c")) for entry in installed}
    if len(identities) != 1:
        raise ValueError(f"ambiguous installed library identity: {origin}")
    version, commit = identities.pop()
    if not commit or len(commit) != 40 or any(char not in "0123456789abcdef" for char in commit):
        raise ValueError(f"missing Alpine recipe commit: {origin}")
    directory = sources / origin
    directory.mkdir()
    archive = directory / "aports.tar.gz"
    recipe_url = f"https://gitlab.alpinelinux.org/alpine/aports/-/archive/{commit}/aports-{commit}.tar.gz"
    recipe_archive_digest = download(recipe_url, archive)
    with tarfile.open(archive) as tar:
        recipes = [member for member in tar.getmembers() if member.name.endswith(f"/{origin}/APKBUILD")]
        if len(recipes) != 1:
            raise ValueError(f"ambiguous Alpine recipe: {origin}")
        prefix = str(Path(recipes[0].name).parent) + "/"
        selected = [member for member in tar.getmembers() if member.name.startswith(prefix)]
        unpacked = directory / "recipe"
        unpacked.mkdir()
        tar.extractall(unpacked, members=selected, filter="data")
    recipe = unpacked / Path(recipes[0].name).parent
    environment = dict(os.environ, SRCDEST=str(directory / "distfiles"))
    subprocess.run(["abuild", "-F", "fetch", "verify", "unpack"], cwd=recipe, env=environment, check=True)
    notices = copy_licences(recipe / "src", licences / origin)
    return {
        "name": origin, "version": version, "recipe_commit": commit,
        "recipe_source": recipe_url, "recipe_archive_sha256": recipe_archive_digest,
        "licence_expressions": sorted({entry["L"] for entry in installed}),
        "licences": notices,
    }


def linked_archives(upstream, installed, expected):
    """Checks the native static link closure against the locked sources and APKs."""
    link_map = upstream / "runtime-link.map"
    paths = sorted(set(re.findall(r"(/[^\s()]+\.a)(?:\(|\s)", link_map.read_text())))
    if not paths:
        raise ValueError("the native runtime has no static link map")
    built = {"libfuse3.a": "libfuse", "libsquashfuse.a": "squashfuse", "libsquashfuse_ll.a": "squashfuse"}
    owners = {f"{entry['P']}-{entry['V']}": entry for entry in installed}
    result = []
    for name in paths:
        path = Path(name).resolve(strict=True)
        if path.name in built:
            source = built[path.name]
        else:
            output = subprocess.run(["apk", "info", "--who-owns", str(path)], check=True, text=True, capture_output=True).stdout.strip()
            owner = output.rsplit(" is owned by ", 1)[-1]
            if owner not in owners or owners[owner]["P"] not in expected:
                raise ValueError(f"unlocked static archive: {name}: {output}")
            source = owners[owner]["o"]
        result.append({"path": str(path), "sha256": digest(path), "source": source})
    return result


def main():
    """Writes the native runtime, source archive inputs and dependency inventory."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--upstream", required=True, type=Path)
    parser.add_argument("--inputs", required=True, type=Path)
    arguments = parser.parse_args()
    arguments.output.mkdir()
    sources = arguments.output / "sources"
    licences = arguments.output / "licences"
    sources.mkdir()
    licences.mkdir()
    inputs = json.loads(arguments.inputs.read_text())
    shutil.copytree(arguments.upstream, sources / "type2-runtime", ignore=shutil.ignore_patterns("out"))
    runtime = list((arguments.upstream / "out").glob("runtime-*"))
    runtime = [path for path in runtime if path.suffix != ".debug"]
    if len(runtime) != 1:
        raise ValueError("expected one native runtime executable")
    shutil.copy2(runtime[0], arguments.output / "runtime")
    copy_licences(sources / "type2-runtime", licences / "type2-runtime")
    packages = installed_packages()
    dependencies = [archive_source(item, sources, licences) for item in inputs["source_dependencies"]]
    locked = inputs["static_packages"]
    expected = {entry["name"]: entry for entry in locked}
    selected = [entry for entry in packages if entry["P"] in expected]
    if {entry["P"] for entry in selected} != set(expected):
        raise ValueError("a pinned static dependency is not installed")
    for entry in selected:
        reference = expected[entry["P"]]
        if entry["V"] != reference["version"] or entry["c"] != reference["recipe_commit"]:
            raise ValueError(f"installed static dependency differs from its lock: {entry['P']}")
    origins = sorted({entry["origin"] for entry in locked})
    dependencies += [alpine_source(name, selected, sources, licences) for name in origins]
    archives = linked_archives(arguments.upstream, packages, expected)
    inventory = {
        "runtime_source_commit": inputs["source_commit"],
        "builder_image": inputs["builder_image"],
        "runtime_sha256": digest(arguments.output / "runtime"),
        "dependencies": dependencies,
        "linked_archives": archives,
    }
    (arguments.output / "runtime-inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
    shutil.copy2(arguments.inputs, sources / "inputs.json")


if __name__ == "__main__":
    main()
