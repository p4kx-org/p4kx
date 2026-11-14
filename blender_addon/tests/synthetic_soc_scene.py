# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Synthetic authored SOC scene data."""


def cell(value, encrypted=False):
    return {"state": "present", "value": value, "containsEncryptedSource": encrypted}


def translation(x, y, z):
    return cell([[1, 0, 0, x], [0, 1, 0, y], [0, 0, 1, z]])


def scene():
    room = {"ordinal": 0, "guid": cell("fixture"), "pivotRelativePosition": cell([5, 6, 7]),
            "pivotRelativeRotation": cell([1, 0, 0, 0])}
    segment = {"ordinal": 2, "localTransform": cell({"rotation_wxyz": [1, 0, 0, 0],
                                                        "translation": [9, 8, 7]}, True)}
    door = {"ordinal": 4, "components": [{"kind": "door"}], "localTransform": translation(4, 2, 3),
            "areas": [{"ordinal": 1, "segments": [segment]}]}
    sensor = {"ordinal": 5, "components": [], "classSensor": cell(True), "localTransform": translation(1, 2, 3),
              "areas": []}
    root = {"placement": None, "children": [{"localTransform": translation(8, 3, 2)}],
            "registry": {"state": "present", "data": {"rooms": [room]}},
            "streams": [{"member": {"state": "present", "data": [{"state": "present",
                                                                   "entities": [door, sensor]}]}}]}
    child = {"placement": [0, 0], "children": [], "registry": {"state": "missing"}, "streams": []}
    return {"format": "p4kx-soc-scene", "schemaVersion": 1, "occurrences": [root, child]}
