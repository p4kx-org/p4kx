# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Verifies exact worn mount fields and their GLB binding in package reports."""

from __future__ import annotations

import json
import struct
import tempfile
import unittest
from pathlib import Path

from p4kx_importer.package_digests import RECEIPT_NAME, file_sha256
from p4kx_importer.validate_package import validate_package
from synthetic_character import character_manifest, rig_glb

_GLB = "meshes/m0001.glb"
_CHILD = "/nodes/3/children/0"
_TRANSLATION = "/nodes/1/translation"
_ROTATION = "/nodes/1/rotation"
_SCALE = "/nodes/1/scale"


def _mounted_rig(skinned: bool, ambiguous_parent: bool) -> bytes:
    raw = rig_glb()
    length, kind = struct.unpack_from("<I4s", raw, 12)
    document = json.loads(raw[20 : 20 + length])
    binary = raw[20 + length :]
    document["nodes"].insert(1, {
        "name": "rigid gear",
        "mesh": 0,
        "translation": [0.5, 0, 0],
        "rotation": [0, 0, 0.70710678, 0.70710678],
        "scale": [1, 2, 1],
    })
    document["nodes"][2]["children"] = [3, 1] if ambiguous_parent else [3]
    document["nodes"][3]["children"] = [1]
    document["scenes"][0]["nodes"] = [0, 2]
    document["animations"][0]["channels"][0]["target"]["node"] = 3
    if skinned:
        document["skins"][0].update(joints=[2, 3], skeleton=2)
    else:
        del document["skins"]
        del document["nodes"][0]["skin"]

    payload = json.dumps(document).encode()
    payload += b" " * (-len(payload) % 4)
    header = struct.pack("<4sIII4s", b"glTF", 2, 20 + len(payload) + len(binary), len(payload), kind)
    return header + payload + binary


class WornMountProvenanceTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="worn-mount-")
        self.addCleanup(temporary.cleanup)
        self.folder = Path(temporary.name)
        self.scene = self.folder / "scene.json"
        self.glb = self.folder / _GLB
        self.glb.parent.mkdir()
        self.data = character_manifest(version="1.21")
        self.data["nodes"] = self.data["nodes"][:1]
        self.data["meshes"] = self.data["meshes"][:1]
        self.rig = self.data["meshes"][0]["rig"]

    def write_fixture(self, *, skinned: bool = True, ambiguous_parent: bool = False) -> dict:
        self.glb.write_bytes(_mounted_rig(skinned, ambiguous_parent))
        self.rig["skinned_meshes"] = int(skinned)
        origin = {"containsEncryptedSource": True, "sourceSetComplete": False, "sourceUsesRefs": ["use"]}
        self.rig["worn_mount_provenance"] = proof = {
            "glb": _GLB,
            "artifact": {"byteLength": str(self.glb.stat().st_size), "sha256": file_sha256(self.glb)},
            "sourceInventory": {
                "sources": [{"id": "entry", "ancestors": [], "storedEncryption": "encrypted"}],
                "uses": [{
                    "id": "use", "source": "entry", "scope": "payload", "role": "conversion",
                    "effectiveEncryption": "encrypted",
                    "location": {"type": "dataForge", "recordIndex": 0, "field": "mount"},
                }],
            },
            "fieldOrigins": {address: dict(origin) for address in (_CHILD, _TRANSLATION, _ROTATION, _SCALE)},
        }
        self.save_manifest()
        return proof

    def save_manifest(self) -> None:
        self.scene.write_text(json.dumps(self.data), encoding="utf-8")

    def write_receipt(self) -> None:
        rows = [{"path": name, "sha256": file_sha256(self.folder / name)} for name in ("scene.json", _GLB)]
        (self.folder / RECEIPT_NAME).write_text(json.dumps({"schema": 1, "files": rows}), encoding="utf-8")

    def assert_mesh_problem(self, message: str) -> dict:
        report = validate_package(self.scene)
        self.assertEqual(report["structure"]["status"], "failed")
        self.assertIn(message, report["structure"]["mesh_problems"][0]["detail"])
        return report

    def test_checks_mounts_with_and_without_a_skin(self) -> None:
        for skinned in (True, False):
            with self.subTest(skinned=skinned):
                self.write_fixture(skinned=skinned)
                self.write_receipt()
                report = validate_package(self.scene)
                self.assertEqual(report["structure"]["status"], "valid")
                self.assertEqual(report["digests"]["status"], "verified")

    def test_legacy_package_can_omit_worn_mount_proof(self) -> None:
        self.write_fixture()
        self.data["version"] = "1.20"
        del self.rig["worn_mount_provenance"]
        self.save_manifest()
        self.assertEqual(validate_package(self.scene)["structure"]["status"], "valid")

    def test_rehashed_receipt_does_not_validate_the_old_proof_after_glb_changes(self) -> None:
        self.write_fixture()
        changed = self.glb.read_bytes().replace(b"rigid gear", b"rigid tool", 1)
        self.glb.write_bytes(changed)
        self.write_receipt()
        report = self.assert_mesh_problem("Worn mount provenance GLB length or SHA-256 differs.")
        self.assertEqual(report["digests"]["status"], "verified")

    def test_mount_child_must_be_a_rigid_root_at_the_observed_bone_slot(self) -> None:
        for address, message in (
            ("/nodes/0/children/0", "parent is not a GLB bone"),
            ("/nodes/3/children/1", "child position is outside the GLB bone"),
            ("/nodes/2/children/0", "child is not a unique rigid GLB root"),
        ):
            with self.subTest(address=address):
                fields = self.write_fixture()["fieldOrigins"]
                fields[address] = fields.pop(_CHILD)
                self.save_manifest()
                self.assert_mesh_problem(message)

    def test_mount_child_cannot_have_another_parent(self) -> None:
        self.write_fixture(ambiguous_parent=True)
        self.assert_mesh_problem("child has ambiguous GLB parentage")

    def test_requires_proof_for_every_emitted_local_transform(self) -> None:
        fields = self.write_fixture()["fieldOrigins"]
        del fields[_ROTATION]
        self.save_manifest()
        self.assert_mesh_problem("missing an emitted GLB root transform")

    def test_transform_address_must_belong_to_its_observed_mount(self) -> None:
        fields = self.write_fixture()["fieldOrigins"]
        fields["/nodes/3/translation"] = fields.pop(_TRANSLATION)
        self.save_manifest()
        self.assert_mesh_problem("transform is absent from its GLB root")

    def test_origin_references_are_verified_for_bound_fields(self) -> None:
        fields = self.write_fixture()["fieldOrigins"]
        fields[_SCALE]["sourceUsesRefs"] = ["missing"]
        self.save_manifest()
        report = validate_package(self.scene)
        self.assertEqual(report["structure"]["status"], "failed")
        self.assertIn("unresolved contributor references", report["structure"]["manifest_error"])
