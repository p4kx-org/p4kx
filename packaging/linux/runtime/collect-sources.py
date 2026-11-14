"""Collects the corresponding source and original licences of a static AppImage runtime."""

import argparse
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import tarfile
import time
import urllib.error
import urllib.request

import build_inputs
from build_inputs import digest


def download(url, destination, expected=None):
    """Downloads an HTTPS source file and checks its expected digest."""
    if not url.startswith("https://"):
        raise ValueError("source downloads require HTTPS")
    for attempt in range(4):
        try:
            with urllib.request.urlopen(url, timeout=120) as response, destination.open("wb") as output:
                shutil.copyfileobj(response, output)
            break
        except (urllib.error.URLError, TimeoutError, ConnectionError) as error:
            if isinstance(error, urllib.error.HTTPError) and error.code not in (408, 429, 500, 502, 503, 504):
                raise
            if attempt == 3:
                raise
            time.sleep(2 ** attempt)
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


def recipe_source(commit, recipes):
    """Requires an exact official archive URL, root and digest for a recipe commit."""
    reference = recipes.get(commit)
    urls = {
        f"https://github.com/alpinelinux/aports/archive/{commit}.tar.gz",
        f"https://gitlab.alpinelinux.org/api/v4/projects/alpine%2Faports/repository/archive.tar.gz?sha={commit}",
    }
    roots = {f"aports-{commit}", f"aports-{commit}-{commit}"}
    if not reference or reference.get("url") not in urls or reference.get("archive_root") not in roots:
        raise ValueError(f"unlocked Alpine recipe archive: {commit}")
    if not re.fullmatch(r"[0-9a-f]{64}", reference.get("sha256", "")):
        raise ValueError(f"invalid Alpine recipe digest: {commit}")
    return reference


def preload_distfile(origin, recipe, distfiles):
    """Preloads the locked official distfile for the installed recipe origin."""
    if "distfile" not in recipe:
        return None
    pinned = recipe["distfile"]
    name = pinned.get("name", "")
    expected = pinned.get("sha256", "")
    if (pinned.get("origin") != origin
            or not re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9._+-]*", name)
            or not re.fullmatch(r"[0-9a-f]{64}", expected)
            or pinned.get("url") != f"https://distfiles.alpinelinux.org/distfiles/v3.23/{name}"):
        raise ValueError(f"invalid Alpine distfile pin: {origin}")
    distfiles.mkdir()
    actual = download(pinned["url"], distfiles / name, expected)
    return {"name": name, "url": pinned["url"], "sha256": actual}


def alpine_source(origin, packages, sources, licences, recipes):
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
    reference = recipe_source(commit, recipes)
    recipe_url = reference["url"]
    recipe_archive_digest = download(recipe_url, archive, reference["sha256"])
    with tarfile.open(archive) as tar:
        members = tar.getmembers()
        root = reference["archive_root"]
        if any(member.name != root and not member.name.startswith(root + "/") for member in members):
            raise ValueError(f"Alpine source archive root mismatch: {commit}")
        matches = [member for member in members if member.name.endswith(f"/{origin}/APKBUILD")]
        if len(matches) != 1:
            raise ValueError(f"ambiguous Alpine recipe: {origin}")
        prefix = PurePosixPath(matches[0].name).parent.as_posix() + "/"
        selected = [member for member in members if member.name.startswith(prefix)]
        unpacked = directory / "recipe"
        unpacked.mkdir()
        tar.extractall(unpacked, members=selected, filter="data")
    recipe = unpacked / Path(matches[0].name).parent
    distfiles = directory / "distfiles"
    preloaded = preload_distfile(origin, reference, distfiles)
    environment = dict(os.environ, SRCDEST=str(distfiles))
    subprocess.run(["abuild", "-F", "fetch", "verify", "unpack"], cwd=recipe, env=environment, check=True)
    notices = copy_licences(recipe / "src", licences / origin)
    result = {
        "name": origin, "version": version, "recipe_commit": commit,
        "recipe_source": recipe_url, "recipe_archive_sha256": recipe_archive_digest,
        "licence_expressions": sorted({entry["L"] for entry in installed}),
        "licences": notices,
    }
    if preloaded is not None:
        result["distfile"] = preloaded
    return result


def linked_archives(upstream, installed, expected, built):
    """Checks the native static link closure against the locked sources and APKs."""
    link_map = upstream / "runtime-link.map"
    paths = sorted(set(re.findall(r"(/[^\s()]+\.a)(?:\(|\s)", link_map.read_text())))
    if not paths:
        raise ValueError("the native runtime has no static link map")
    owners = {f"{entry['P']}-{entry['V']}": entry for entry in installed}
    result = []
    for name in paths:
        path = Path(name).resolve(strict=True)
        bound = build_inputs.bound_archive(path, built)
        if bound:
            source = bound["source"]
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
    parser.add_argument("--dependencies", required=True, type=Path)
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
    built = build_inputs.validate(inputs, arguments.dependencies)
    shutil.copytree(arguments.dependencies, sources / "dependency-build", symlinks=True)
    dependencies = []
    for name, source in built["sources"].items():
        notices = copy_licences(arguments.dependencies / source["directory"], licences / name)
        dependencies.append({**source, "source_sha256": source["sha256"], "licences": notices})
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
    dependencies += [alpine_source(name, selected, sources, licences, inputs["recipe_archives"]) for name in origins]
    archives = linked_archives(arguments.upstream, packages, expected, built)
    inventory = {
        "runtime_source_commit": inputs["source_commit"],
        "builder_image": inputs["builder_image"],
        "runtime_sha256": digest(arguments.output / "runtime"),
        "dependencies": dependencies,
        "linked_archives": archives,
        "build_inputs": built,
        "original_runtime_makefile_sha256": digest(arguments.dependencies / "upstream-runtime-Makefile"),
        "runtime_makefile_sha256": digest(arguments.upstream / "src/runtime/Makefile"),
    }
    (arguments.output / "runtime-inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
    shutil.copy2(arguments.inputs, sources / "inputs.json")


if __name__ == "__main__":
    main()
