# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks material sidecar stream closure and package digest integration."""

import gzip
import json

import pytest

from p4kx_importer.package_digests import file_sha256
from p4kx_importer.validate_package import validate_package
from synthetic import manifest, write_package


FILE = "material-sources/s0000.jsonl.gz"


def _rows():
    return [
        {"type": "header", "format": "p4kx-blender-material-source", "version": 1, "library": 0},
        {"type": "library", "authored": {"root": {"a/b~": ["lexeme"]}}},
        {"type": "source", "source": {"id": "s0", "ancestors": [], "storedEncryption": "encrypted"}},
        {
            "type": "sourceUse",
            "sourceUse": {
                "id": "u0", "source": "s0", "scope": "payload", "role": "payload",
                "effectiveEncryption": "encrypted",
                "location": {"type": "subresource", "namespace": "synthetic", "indices": ["0"]},
            },
        },
        {
            "type": "field", "path": "/authored/root/a~1b~0/0",
            "containsEncryptedSource": True, "sourceSetComplete": True, "sourceUsesRefs": ["u0"],
        },
        {"type": "end", "sources": 1, "uses": 1, "fields": 1},
    ]


def _package(folder, reference=FILE):
    scene = manifest(version="1.20", material_libraries=[
        {"path": "library", "authored_file": {"file": reference, "schema_version": 1}},
    ])
    return write_package(folder, scene)


def _write(folder, rows):
    path = folder / FILE
    path.parent.mkdir(exist_ok=True)
    content = "".join(json.dumps(row) + "\n" for row in rows)
    path.write_bytes(gzip.compress(content.encode("utf-8"), mtime=0))
    return path


def test_authored_material_companion_contributes_to_package_receipt(tmp_path):
    scene = _package(tmp_path)
    _write(tmp_path, _rows())
    names = [path.relative_to(tmp_path).as_posix() for path in tmp_path.rglob("*") if path.is_file()]
    receipt = tmp_path / "receipt.json"
    receipt.write_text(
        json.dumps({"schema": 1, "files": [
            {"path": name, "sha256": file_sha256(tmp_path / name)} for name in names
        ]}),
        encoding="utf-8",
    )
    report = validate_package(scene)
    assert report["structure"]["status"] == "valid"
    assert report["digests"]["status"] == "verified"
    assert FILE in report["digests"]["verified"]


@pytest.mark.parametrize(("change", "message"), [
    (lambda rows: rows[0].update(library=1), "library ordinal"),
    (lambda rows: rows[3]["sourceUse"].update(source="unseen"), "source use"),
    (lambda rows: rows[4].update(path="/authored/root/a~1b~0/01"), "field path"),
    (lambda rows: rows[4].update(sourceUsesRefs=["unseen"]), "contributor"),
    (lambda rows: rows[4].update(containsEncryptedSource=False), "encryption marker"),
    (lambda rows: rows.insert(5, rows[4]), "field path"),
    (lambda rows: rows[-1].update(fields=2), "counts"),
    (lambda rows: rows.pop(), "no end row"),
])
def test_rejects_invalid_authored_material_stream(tmp_path, change, message):
    scene = _package(tmp_path)
    rows = _rows()
    change(rows)
    _write(tmp_path, rows)
    result = validate_package(scene)["structure"]
    assert result["status"] == "failed"
    assert message in result["asset_problems"][0]["detail"]


def test_rejects_noncanonical_or_missing_authored_material_file(tmp_path):
    scene = _package(tmp_path, "material-sources/s0001.jsonl.gz")
    result = validate_package(scene)["structure"]
    assert result["status"] == "failed"
    assert "reference is invalid" in result["manifest_error"]
    scene = _package(tmp_path)
    result = validate_package(scene)["structure"]
    assert result["status"] == "failed"
    assert result["asset_problems"][0]["path"] == FILE


def test_rejects_truncated_gzip_even_with_available_end_row(tmp_path):
    scene = _package(tmp_path)
    artifact = _write(tmp_path, _rows())
    artifact.write_bytes(artifact.read_bytes()[:-4])
    result = validate_package(scene)["structure"]
    assert result["status"] == "failed"
    assert "cannot be read" in result["asset_problems"][0]["detail"]
