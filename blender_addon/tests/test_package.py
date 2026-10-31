# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Tests of the pure package reader: parsing, version rule and refusal of unsafe input."""

from __future__ import annotations

import copy
import json
from pathlib import Path
from typing import Any

import pytest

from p4kx_importer import manifest_fields, package
from p4kx_importer.package import PackageError, PackageTooNewError
from synthetic import glb, manifest, write_package


def parse(data: dict[str, Any], root: Path = Path(".")) -> package.Package:
    return package.parse_manifest(data, root)


def test_reads_a_valid_package() -> None:
    result = parse(manifest())
    assert result.version == (1, 0)
    assert result.warnings == []
    assert result.entity_name == "TEST_Ship"
    assert [node.name for node in result.nodes][:2] == ["TEST_Ship", "hardpoint_a/Gun"]
    assert result.nodes[1].parent == 0
    assert result.nodes[1].mesh == "m0001"
    assert result.meshes["m0001"].materials[0].textures == {"diffuse": "t0001", "normal": "t0002"}
    assert result.textures["t0002"].colour_space == "linear"


def test_loads_from_folder_and_from_scene_json(tmp_path: Path) -> None:
    scene = write_package(tmp_path)
    assert package.load_package(tmp_path).root == tmp_path
    assert package.load_package(scene).root == tmp_path


def test_a_higher_major_version_is_refused_with_an_update_message() -> None:
    with pytest.raises(PackageTooNewError, match="Update the P4KX Blender add-on"):
        parse(manifest(version="2.0"))


def test_a_higher_minor_version_reads_with_a_warning() -> None:
    newer = f"1.{package.SUPPORTED_MINOR + 1}"
    result = parse(manifest(version=newer))
    assert len(result.warnings) == 1
    assert newer in result.warnings[0]
    assert "Update" in result.warnings[0]


def test_major_zero_and_malformed_versions_are_refused() -> None:
    for version in ("0.9", "1", "1.0.0", "a.b", 1.0, None):
        with pytest.raises(PackageError):
            parse(manifest(version=version))


def test_unknown_format_basis_and_unit_are_refused() -> None:
    with pytest.raises(PackageError, match="Not a P4KX"):
        parse(manifest(format="other"))
    with pytest.raises(PackageError, match="basis"):
        parse(manifest(coordinates={"basis": "z_up", "unit": "metre"}))
    with pytest.raises(PackageError, match="unit"):
        parse(manifest(coordinates={"basis": "gltf_y_up", "unit": "foot"}))


def test_unknown_fields_and_values_of_a_later_minor_version_are_tolerated() -> None:
    data = manifest(version="1.3", skeletons=[{"id": "s1"}])
    data["nodes"][1]["future"] = {"a": 1}
    data["meshes"][0]["materials"][0]["layers"] = [1, 2]
    data["textures"][0]["unknown"] = True
    data["textures"].append({**data["textures"][0], "id": "t0003", "format": "exr"})
    data["textures"].append({**data["textures"][0], "id": "t0004", "colour_space": "acescg"})
    data["textures"].append({**data["textures"][0], "id": "t0005", "alpha": "roughness"})
    data["meshes"][0]["materials"][0]["textures"]["specular"] = "t0004"
    data["nodes"].append({**data["nodes"][0], "kind": "portal", "name": "L", "parent": 0})
    data["nodes"].append({**data["nodes"][0], "kind": "entity", "name": "child", "parent": 4})
    result = parse(data)
    assert set(result.textures) == {"t0001", "t0002"}
    assert "specular" not in result.meshes["m0001"].materials[0].textures
    assert [node.name for node in result.nodes] == [
        "TEST_Ship",
        "hardpoint_a/Gun",
        "hardpoint_b/Gun",
        "hardpoint_c/Box",
    ]
    assert any("portal" in warning for warning in result.warnings)


@pytest.mark.parametrize(
    "bad_file",
    [
        "../meshes/m0001.glb",
        "meshes/../../m0001.glb",
        "/etc/passwd",
        "C:/Windows/x.glb",
        "meshes\\m0001.glb",
        "meshes/evil.glb",
        "meshes/m0001.glb\n",
        "textures/t0001.png",
        "",
    ],
)
def test_mesh_files_outside_the_package_naming_are_refused(bad_file: str) -> None:
    data = manifest()
    data["meshes"][0]["file"] = bad_file
    with pytest.raises(PackageError):
        parse(data)


@pytest.mark.parametrize(
    "bad_file", ["../t0001.png", "textures/../x.png", "textures/t0001.exe", "meshes/m0001.glb"]
)
def test_texture_files_outside_the_package_naming_are_refused(bad_file: str) -> None:
    data = manifest()
    data["textures"][0]["file"] = bad_file
    with pytest.raises(PackageError):
        parse(data)


def test_resolve_file_refuses_what_escapes_the_folder(tmp_path: Path) -> None:
    root = tmp_path / "pkg"
    root.mkdir()
    assert package.resolve_file(root, "meshes/m0001.glb") == root.resolve() / "meshes" / "m0001.glb"
    for escape in ("../outside.glb", "meshes/../../outside.glb"):
        with pytest.raises(PackageError):
            package.resolve_file(root, escape)


def test_resolve_file_refuses_a_symlink_that_leaves_the_folder(tmp_path: Path) -> None:
    root = tmp_path / "pkg"
    (root / "meshes").mkdir(parents=True)
    outside = tmp_path / "outside"
    outside.mkdir()
    try:
        (root / "meshes" / "m0001.glb").symlink_to(outside / "x.glb")
    except OSError:
        pytest.skip("symlinks are not available")
    with pytest.raises(PackageError):
        package.resolve_file(root, "meshes/m0001.glb")


def test_bad_references_and_values_are_refused() -> None:
    mutations = [
        lambda d: d["nodes"][1].update(parent=5),
        lambda d: d["nodes"][1].update(parent=1),
        lambda d: d["nodes"][1].update(parent=True),
        lambda d: d["nodes"][1].update(mesh="m9999"),
        lambda d: d["nodes"][1].update(translation=[0, 0]),
        lambda d: d["nodes"][1].update(translation=[0, 0, float("nan")]),
        lambda d: d["nodes"][1].update(translation=[0, 0, "1"]),
        lambda d: d["nodes"][1].update(rotation=[0, 0, 0, 0]),
        lambda d: d["nodes"][1].update(scale=[1, 1, float("inf")]),
        lambda d: d["meshes"][0].update(id="../m0001"),
        lambda d: d["meshes"].append(copy.deepcopy(d["meshes"][0])),
        lambda d: d["textures"].append(copy.deepcopy(d["textures"][0])),
        lambda d: d["textures"][0].update(colour_space=5),
        lambda d: d["meshes"][0]["materials"][0].update(index=-1),
        lambda d: d.update(nodes={}),
        lambda d: d.pop("meshes"),
    ]
    for mutate in mutations:
        data = manifest()
        mutate(data)
        with pytest.raises(PackageError):
            parse(data)


def test_a_texture_id_the_package_lacks_is_ignored() -> None:
    data = manifest()
    data["meshes"][0]["materials"][0]["textures"]["specular"] = "t0099"
    assert "specular" not in parse(data).meshes["m0001"].materials[0].textures


def test_rotation_is_normalised() -> None:
    data = manifest()
    data["nodes"][1]["rotation"] = [0, 0, 0, 2]
    assert parse(data).nodes[1].rotation == (0.0, 0.0, 0.0, 1.0)


def test_not_json_and_wrong_file_are_refused(tmp_path: Path) -> None:
    broken = tmp_path / "scene.json"
    broken.write_text("{nope", encoding="utf-8")
    with pytest.raises(PackageError, match="not valid JSON"):
        package.load_package(broken)
    broken.write_text("[" * 100000, encoding="utf-8")
    with pytest.raises(PackageError):
        package.load_package(broken)
    broken.write_text(json.dumps([1]), encoding="utf-8")
    with pytest.raises(PackageError, match="object"):
        package.load_package(broken)
    other = tmp_path / "other.json"
    other.write_text("{}", encoding="utf-8")
    with pytest.raises(PackageError, match="scene.json"):
        package.load_package(other)
    with pytest.raises(PackageError):
        package.load_package(tmp_path / "missing" / "scene.json")


def test_the_manifest_size_is_capped(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    scene = write_package(tmp_path)
    monkeypatch.setattr(package, "MAX_MANIFEST_BYTES", 10)
    with pytest.raises(PackageError, match="larger"):
        package.load_package(scene)


def test_axis_conversion_maps_gltf_y_up_to_blender_z_up() -> None:
    assert package.gltf_to_blender_vector((1.0, 2.0, 3.0)) == (1.0, -3.0, 2.0)
    assert package.gltf_to_blender_scale((1.0, 2.0, 3.0)) == (1.0, 3.0, 2.0)
    assert package.gltf_to_blender_quaternion((0.1, 0.2, 0.3, 0.9)) == (0.9, 0.1, -0.3, 0.2)


def test_blender_name_truncates_on_a_character_boundary() -> None:
    assert package.blender_name("short") == "short"
    long = "é" * 40
    shortened = package.blender_name(long)
    assert len(shortened.encode("utf-8")) <= 63
    assert long.startswith(shortened)


def test_pair_materials_matches_suffixed_duplicates_in_order() -> None:
    pairs = package.pair_materials(
        [(0, "Steel"), (1, "Steel"), (2, "Glass")], ["Glass", "Steel", "Steel.001"]
    )
    assert pairs == {0: "Steel", 1: "Steel.001", 2: "Glass"}
    assert package.pair_materials([(0, "A")], ["B"]) == {}
    assert package.pair_materials([(0, "Steel")], ["Steel.001"]) == {0: "Steel.001"}


def test_pair_materials_matches_names_longer_than_blender_allows() -> None:
    long = "x" * 70
    assert package.pair_materials([(0, long)], ["x" * 63]) == {0: "x" * 63}


def test_a_huge_integer_is_refused_not_an_overflow() -> None:
    data = manifest()
    data["nodes"][1]["translation"] = [0, 0, 10**400]
    with pytest.raises(PackageError, match="finite"):
        parse(data)


def test_the_counts_of_nodes_meshes_and_textures_are_bounded(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(manifest_fields, "MAX_ITEMS", 3)
    with pytest.raises(PackageError, match="more than 3"):
        parse(manifest())


def test_glb_with_embedded_data_passes_the_check(tmp_path: Path) -> None:
    path = tmp_path / "m0001.glb"
    path.write_bytes(glb("m0001", ["Mat"]))
    package.check_glb(path)


@pytest.mark.parametrize("key", ["buffers", "images"])
def test_glb_with_an_external_uri_is_refused(tmp_path: Path, key: str) -> None:
    path = tmp_path / "m0001.glb"
    path.write_bytes(glb("m0001", ["Mat"], external={key: [{"uri": "../secret.bin"}]}))
    with pytest.raises(PackageError, match="external file"):
        package.check_glb(path)


def test_a_broken_glb_is_refused(tmp_path: Path) -> None:
    path = tmp_path / "m0001.glb"
    for content in (b"", b"glTF", b"x" * 40, glb("a", [])[:30]):
        path.write_bytes(content)
        with pytest.raises(PackageError):
            package.check_glb(path)
    with pytest.raises(PackageError):
        package.check_glb(tmp_path / "missing.glb")
