# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks the standalone package report and its independent digest status."""

from __future__ import annotations

import json
from pathlib import Path
from struct import pack_into

import pytest

from p4kx_importer.soc_scene import SOC_SCENE_FILE, SOC_SCENE_VERSION
from p4kx_importer.package_digests import file_sha256
from p4kx_importer.validate_package import main, validate_package
from synthetic import glb, manifest, write_package
from synthetic_soc_scene import scene as authored_scene


def receipt_for(folder: Path, destination: Path, names: list[str]) -> Path:
    destination.write_text(
        json.dumps(
            {"schema": 1, "files": [{"path": name, "sha256": file_sha256(folder / name)} for name in names]},
        ),
        encoding="utf-8",
    )
    return destination


def test_report_is_equal_for_identical_packages_in_two_folders(tmp_path: Path) -> None:
    first = write_package(tmp_path / "a")
    second = write_package(tmp_path / "b")
    first_result = validate_package(first)
    assert first_result == validate_package(second.parent)
    assert first_result["structure"]["status"] == "valid"
    assert first_result["digests"] == {"status": "absent"}


def test_old_package_without_geometry_counts_is_partial(tmp_path: Path) -> None:
    data = manifest(version="1.0")
    del data["meshes"][0]["vertices"]
    del data["meshes"][0]["triangles"]
    report = validate_package(write_package(tmp_path, data))
    assert report["structure"]["status"] == "partial"
    assert report["structure"]["incomplete_mesh_counts"] == ["m0001"]
    assert report["digests"] == {"status": "absent"}


@pytest.mark.parametrize(("key", "message"), [("vertices", "vertex"), ("triangles", "triangle")])
def test_report_refuses_wrong_manifest_geometry_count(tmp_path: Path, key: str, message: str) -> None:
    data = manifest()
    data["meshes"][0][key] += 1
    report = validate_package(write_package(tmp_path, data))
    structure = report["structure"]
    assert structure["status"] == "failed"
    assert structure["checked_meshes"] == ["m0002"]
    assert structure["mesh_problems"][0]["mesh_id"] == "m0001"
    assert f"GLB {message} count" in structure["mesh_problems"][0]["detail"]


def test_vertex_count_is_shared_between_triangle_primitives(tmp_path: Path) -> None:
    data = manifest()
    mesh = data["meshes"][0]
    mesh["materials"].append({"index": 1, "name": "second", "textures": {}})
    mesh["triangles"] = 2
    report = validate_package(write_package(tmp_path, data))
    assert report["structure"]["status"] == "valid"


def test_non_triangle_primitives_are_reported_without_count_comparison(tmp_path: Path) -> None:
    scene = write_package(tmp_path)
    mesh = tmp_path / "meshes/m0001.glb"
    mesh.write_bytes(
        glb(
            "m0001",
            ["Gun_mat"],
            external={"meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "material": 0, "mode": 0}]}]},
        ),
    )
    report = validate_package(scene)
    assert report["structure"]["status"] == "partial"
    assert report["structure"]["incomplete_mesh_counts"] == ["m0001"]


def test_report_checks_unplaced_meshes_and_missing_textures(tmp_path: Path) -> None:
    data = manifest()
    data["meshes"].append({"id": "m0003", "file": "meshes/m0003.glb", "materials": []})
    scene = write_package(tmp_path, data)
    (tmp_path / "meshes/m0003.glb").unlink()
    (tmp_path / "textures/t0001.png").unlink()
    report = validate_package(scene)
    assert report["structure"]["status"] == "failed"
    assert report["structure"]["mesh_problems"][0]["mesh_id"] == "m0003"
    assert {problem["path"] for problem in report["structure"]["asset_problems"]} == {
        "meshes/m0003.glb", "textures/t0001.png",
    }


def test_full_and_partial_receipts_are_distinct_from_structure(tmp_path: Path) -> None:
    folder = tmp_path / "package"
    scene = write_package(folder)
    names = sorted(path.relative_to(folder).as_posix() for path in folder.rglob("*") if path.is_file())
    complete = receipt_for(folder, tmp_path / "complete.json", names)
    partial = receipt_for(folder, tmp_path / "partial.json", ["scene.json"])
    result = validate_package(scene, complete)
    assert result["structure"]["status"] == "valid"
    assert result["digests"]["status"] == "verified"
    assert result["digests"]["verified"] == names
    partial_result = validate_package(scene, partial)
    assert partial_result["structure"] == result["structure"]
    assert partial_result["digests"]["status"] == "partial"
    assert partial_result["digests"]["unrecorded"] == sorted(set(names) - {"scene.json"})


def test_report_checks_declared_soc_companion_and_its_digest(tmp_path: Path) -> None:
    folder = tmp_path / "package"
    data = manifest(version="1.13")
    data["authored_soc"] = {"file": SOC_SCENE_FILE, "schema_version": SOC_SCENE_VERSION}
    scene = write_package(folder, data)

    missing = validate_package(scene)["structure"]
    assert missing["status"] == "failed"
    assert [problem["path"] for problem in missing["asset_problems"]] == [SOC_SCENE_FILE]

    companion = folder / SOC_SCENE_FILE
    companion.parent.mkdir()
    companion.write_text(json.dumps(authored_scene()), encoding="utf-8")
    names = sorted(path.relative_to(folder).as_posix() for path in folder.rglob("*") if path.is_file())
    receipt = receipt_for(folder, tmp_path / "receipt.json", names)
    valid = validate_package(scene, receipt)
    assert valid["structure"]["status"] == "valid"
    assert valid["digests"]["status"] == "verified"
    assert SOC_SCENE_FILE in valid["digests"]["verified"]

    companion.write_text(json.dumps(authored_scene(), indent=2), encoding="utf-8")
    changed = validate_package(scene, receipt)
    assert changed["structure"]["status"] == "valid"
    assert changed["digests"]["mismatches"] == [SOC_SCENE_FILE]

    companion.write_text("{}", encoding="utf-8")
    malformed = validate_package(scene)["structure"]
    assert malformed["status"] == "failed"
    assert [problem["path"] for problem in malformed["asset_problems"]] == [SOC_SCENE_FILE]


def test_rehashed_malformed_glb_remains_structurally_failed(tmp_path: Path) -> None:
    folder = tmp_path / "package"
    scene = write_package(folder)
    mesh = folder / "meshes/m0001.glb"
    content = bytearray(mesh.read_bytes())
    pack_into("<I", content, 4, 9)
    mesh.write_bytes(content)
    names = sorted(path.relative_to(folder).as_posix() for path in folder.rglob("*") if path.is_file())
    receipt = receipt_for(folder, tmp_path / "receipt.json", names)
    result = validate_package(scene, receipt)
    assert result["structure"]["status"] == "failed"
    assert result["digests"]["status"] == "verified"


def test_digest_receipt_cannot_select_a_file_outside_package(tmp_path: Path) -> None:
    folder = tmp_path / "package"
    scene = write_package(folder)
    outside = tmp_path / "outside.bin"
    outside.write_bytes(b"outside")
    receipt = receipt_for(folder, tmp_path / "receipt.json", ["../outside.bin", "scene.json"])
    report = validate_package(scene, receipt)
    assert report["structure"]["status"] == "valid"
    assert report["digests"]["status"] == "failed"
    assert report["digests"]["unknown"] == ["../outside.bin"]
    assert report["digests"]["verified"] == ["scene.json"]


def test_command_returns_structured_failure_for_bad_manifest(tmp_path: Path, monkeypatch, capsys) -> None:
    scene = write_package(tmp_path)
    scene.write_text("{}", encoding="utf-8")
    monkeypatch.setattr("sys.argv", ["validate_package", str(scene)])
    assert main() == 1
    report = json.loads(capsys.readouterr().out)
    assert report["schema"] == 1
    assert report["structure"]["status"] == "failed"
    assert "format" in report["structure"]["manifest_error"]
