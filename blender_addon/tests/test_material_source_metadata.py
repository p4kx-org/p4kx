# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Additive source metadata leaves the supported rendering declarations unchanged."""

from pathlib import Path

from p4kx_importer import package
from synthetic import manifest


def test_library_source_metadata_is_additive_and_does_not_override_drawn_materials() -> None:
    baseline = manifest(version="1.9")
    enriched = manifest(version="1.9")
    enriched["material_libraries"] = [
        {"path": "synthetic/library", "authored": {"encoding": "text", "root": {"MtlFlags": "-1"}}}
    ]
    enriched["meshes"][0]["materials"][0]["source_library"] = 0
    plain = package.parse_manifest(baseline, Path("."))
    loaded = package.parse_manifest(enriched, Path("."))
    assert loaded.warnings == []
    assert loaded.meshes == plain.meshes
    assert loaded.material_descriptions == plain.material_descriptions
    assert loaded.nodes == plain.nodes
