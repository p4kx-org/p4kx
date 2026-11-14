# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Exercises bounded GLB structure and package mesh bindings."""

from __future__ import annotations

from io import BytesIO
from pathlib import Path
from struct import pack, pack_into, unpack_from

import pytest

from p4kx_importer import glb_preflight, package
from p4kx_importer.mesh_preflight import preflight_meshes
from synthetic import TRIANGLE, glb, manifest, write_package


def with_header(data: bytes, offset: int, value: int) -> bytes:
    patched = bytearray(data)
    pack_into("<I", patched, offset, value)
    return bytes(patched)


def with_chunks(data: bytes, chunks: bytes) -> bytes:
    return with_header(data + chunks, 8, len(data) + len(chunks))


def without_bin(data: bytes) -> bytes:
    json_length = unpack_from("<I", data, 12)[0]
    end = 20 + json_length
    return with_header(data[:end], 8, end)


@pytest.mark.parametrize(
    ("offset", "value", "message"),
    [
        (4, 9, "Unsupported GLB version"),
        (8, 300, "total length"),
        (12, 5, "not aligned"),
        (12, 0xFFFFFFFC, "extends past"),
    ],
)
def test_glb_header_rejects_inconsistent_fields(
    tmp_path: Path, offset: int, value: int, message: str
) -> None:
    path = tmp_path / "mesh.glb"
    path.write_bytes(with_header(glb("mesh", ["mat"]), offset, value))
    with pytest.raises(package.PackageError, match=message):
        package.check_glb(path)


def test_glb_json_budget_checks_padded_size_before_reading(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    content = glb("mesh", ["mat"])
    json_length = unpack_from("<I", content, 12)[0]
    monkeypatch.setattr(glb_preflight, "MAX_GLB_JSON_BYTES", json_length)
    assert glb_preflight._check_stream(BytesIO(content)).bytes == len(content)

    class HeaderReader(BytesIO):
        def read(self, size: int = -1) -> bytes:
            assert size >= 0 and self.tell() + size <= 20
            return super().read(size)

    budget = json_length - 4
    monkeypatch.setattr(glb_preflight, "MAX_GLB_JSON_BYTES", budget)
    message = f"JSON chunk needs {json_length} bytes, exceeding the package budget of {budget} bytes"
    with pytest.raises(package.PackageError, match=message):
        glb_preflight._check_stream(HeaderReader(content))

    no_json_chunk = with_header(content, 16, int.from_bytes(b"BIN\0", byteorder="little"))
    with pytest.raises(package.PackageError, match="no JSON chunk in the first position"):
        glb_preflight._check_stream(HeaderReader(no_json_chunk))


@pytest.mark.parametrize(
    ("overrides", "message"),
    [
        ({"asset": {"version": "3.0"}}, "Unsupported glTF asset version"),
        ({"buffers": [{"byteLength": len(TRIANGLE) - 4}]}, "does not match"),
        ({"bufferViews": [{"buffer": 0, "byteOffset": 4, "byteLength": len(TRIANGLE)}]}, "exceeds its buffer"),
        ({"accessors": [{"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3"}]}, "exceeds its bufferView"),
        ({"accessors": [{"bufferView": 0, "byteOffset": 1, "componentType": 5126, "count": 1, "type": "VEC3"}]}, "misaligned"),
        ({"meshes": [{"primitives": [{"attributes": {"POSITION": 2}}]}]}, "primitive attribute accessor index"),
        ({"meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "material": 2}]}]}, "primitive material index"),
        ({"nodes": [{"mesh": 3}]}, "node mesh index"),
        ({"images": [{"bufferView": 2}]}, "image bufferView index"),
        ({"images": [{"uri": "data:image/png;base64,AAAA"}]}, "external file"),
    ],
)
def test_glb_reports_dangling_references_and_out_of_range_storage(
    tmp_path: Path, overrides: dict, message: str
) -> None:
    path = tmp_path / "mesh.glb"
    path.write_bytes(glb("mesh", ["mat"], external=overrides))
    with pytest.raises(package.PackageError, match=message):
        package.check_glb(path)


def test_glb_requires_declared_bin_and_rejects_bad_padding(tmp_path: Path) -> None:
    path = tmp_path / "mesh.glb"
    content = glb("mesh", ["mat"])
    path.write_bytes(without_bin(content))
    with pytest.raises(package.PackageError, match="missing the declared BIN"):
        package.check_glb(path)

    smaller = glb(
        "mesh",
        ["mat"],
        external={
            "buffers": [{"byteLength": len(TRIANGLE) - 1}],
            "bufferViews": [{"buffer": 0, "byteLength": len(TRIANGLE) - 1}],
            "accessors": [{"bufferView": 0, "componentType": 5126, "count": 2, "type": "VEC3"}],
        },
    )
    path.write_bytes(smaller[:-1] + b"\xff")
    with pytest.raises(package.PackageError, match="BIN padding"):
        package.check_glb(path)


def test_glb_ignores_unknown_chunks_after_bin_and_rejects_repeated_bin(tmp_path: Path) -> None:
    path = tmp_path / "mesh.glb"
    content = glb("mesh", ["mat"])
    path.write_bytes(with_chunks(content, pack("<I4s", 4, b"EXT0") + b"data"))
    assert package.check_glb(path).materials == 1
    path.write_bytes(with_chunks(content, pack("<I4s", 0, b"BIN\x00")))
    with pytest.raises(package.PackageError, match="duplicated or out of order"):
        package.check_glb(path)


def test_glb_accepts_json_only_empty_scene(tmp_path: Path) -> None:
    path = tmp_path / "empty.glb"
    content = glb(
        "empty", [],
        external={"nodes": [], "scenes": [{"nodes": []}], "meshes": [], "accessors": [], "bufferViews": [], "buffers": []},
    )
    path.write_bytes(without_bin(content))
    assert package.check_glb(path).materials == 0


def test_glb_validates_sparse_accessor_bounds_with_matrix_padding(tmp_path: Path) -> None:
    path = tmp_path / "sparse.glb"
    sparse = {
        "count": 2,
        "indices": {"componentType": 5121, "bufferView": 0},
        "values": {"bufferView": 1},
    }
    overrides = {
        "accessors": [{"count": 2, "componentType": 5121, "type": "MAT3", "sparse": sparse}],
        "bufferViews": [
            {"buffer": 0, "byteLength": 2},
            {"buffer": 0, "byteOffset": 4, "byteLength": 23},
        ],
    }
    path.write_bytes(glb("sparse", ["mat"], external=overrides))
    assert package.check_glb(path).materials == 1
    overrides["bufferViews"][1]["byteLength"] = 22
    path.write_bytes(glb("sparse", ["mat"], external=overrides))
    with pytest.raises(package.PackageError, match=r"sparse.values exceeds"):
        package.check_glb(path)


def test_glb_seeks_over_binary_payload_without_loading_it() -> None:
    content = glb("mesh", ["mat"])
    json_length = unpack_from("<I", content, 12)[0]
    bin_offset = 20 + json_length + 8

    class HeaderReader(BytesIO):
        def read(self, size: int = -1) -> bytes:
            assert size >= 0
            assert self.tell() + size <= bin_offset
            return super().read(size)

    info = glb_preflight._check_stream(HeaderReader(content))
    assert info.bytes == len(content)


@pytest.mark.parametrize("version", ["1.0", "1.10"])
def test_preflight_results_are_path_independent_for_valid_old_and_current_packages(
    tmp_path: Path, version: str
) -> None:
    observations = []
    for name in ("first", "second"):
        scene = write_package(tmp_path / name, manifest(version=version))
        result = preflight_meshes(package.load_package(scene))
        observations.append(
            (tuple(result.paths), result.problems, tuple(package.check_glb(path) for path in result.paths.values()))
        )
    assert observations[0] == observations[1]
    assert observations[0][:2] == (("m0001", "m0002"), ())


def test_preflight_reports_manifest_bindings_and_leaves_valid_mesh_available(tmp_path: Path) -> None:
    data = manifest()
    data["meshes"][0]["materials"][0]["index"] = 9
    scene = write_package(tmp_path, data)
    loaded = package.load_package(scene)
    result = preflight_meshes(loaded)
    assert tuple(result.paths) == ("m0002",)
    assert len(result.problems) == 1
    assert result.problems[0].mesh_id == "m0001"
    assert "manifest material index 9" in result.problems[0].detail
    assert loaded.warnings == []


def test_preflight_checks_known_size_only_for_used_meshes(tmp_path: Path) -> None:
    data = manifest()
    data["meshes"][0]["bytes"] = 123
    data["meshes"].append(
        {"id": "m9999", "file": "meshes/m9999.glb", "materials": []},
    )
    scene = write_package(tmp_path, data)
    (tmp_path / "meshes/m9999.glb").unlink()
    result = preflight_meshes(package.load_package(scene))
    assert tuple(result.paths) == ("m0002",)
    assert len(result.problems) == 1
    assert result.problems[0].mesh_id == "m0001"
    assert "size does not match" in result.problems[0].detail


@pytest.mark.parametrize("asset", [{"version": "2.1"}, {"version": "2.1", "minVersion": "2.0"}])
def test_glb_accepts_a_compatible_asset_minor(tmp_path: Path, asset: dict) -> None:
    path = tmp_path / "compatible.glb"
    path.write_bytes(glb("mesh", ["mat"], external={"asset": asset}))
    assert package.check_glb(path).materials == 1
