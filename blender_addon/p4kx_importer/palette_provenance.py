# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Validates provenance for package palette values and node selections."""

from typing import Any

from .manifest_fields import PackageError, read_array, read_object
from .source_evidence import SourceEvidence, follows_pointer

_KINDS = ("named", "materialDefault", "includedPath", "socEntity", "unknown")


def validate(value: object, manifest: dict[str, Any]) -> None:
    """Check the palette, node, field and source-use references of an optional companion."""
    if value is None:
        return
    section = read_object(value, "palette provenance")
    evidence = SourceEvidence.read(section.get("sourceInventory"), "palette")
    palettes = read_array(manifest.get("palettes"), "palettes")
    payloads = read_array(section.get("payloads"), "palette provenance payloads")
    selections = read_array(section.get("selections"), "palette provenance selections")
    palette_ids = [_payload(row, palettes, evidence) for row in payloads]
    for row in selections:
        _selection(row, evidence)
    nodes = read_array(manifest.get("nodes"), "nodes")
    meshes = {
        mesh["id"]: mesh for item in read_array(manifest.get("meshes"), "meshes")
        if isinstance(mesh := item, dict) and isinstance(mesh.get("id"), str)
    }
    _bindings(section.get("bindings"), nodes, meshes, palette_ids, len(selections))


def _payload(value: object, palettes: list[Any], evidence: SourceEvidence) -> str:
    payload = read_object(value, "palette provenance payload")
    index = _index(payload.get("palette"), len(palettes), "palette")
    palette = read_object(palettes[index], "proved palette")
    ident = palette.get("id")
    if not isinstance(ident, str) or not ident:
        raise PackageError("A proved palette has no package identity.")
    if "record" in payload:
        _record(payload["record"], evidence)
    fields = read_object(payload.get("fieldOrigins"), "palette field origins")
    for pointer, origin in fields.items():
        if not isinstance(pointer, str) or not pointer or not follows_pointer(palette, pointer):
            raise PackageError("A palette field path is unresolved.")
        evidence.check_origin(origin)
    return ident


def _selection(value: object, evidence: SourceEvidence) -> None:
    selection = read_object(value, "palette provenance selection")
    if selection.get("kind") not in _KINDS:
        raise PackageError("A palette selection has an unknown kind.")
    evidence.check_origin(selection.get("selector"))
    if "lookup" in selection:
        lookup = read_object(selection["lookup"], "palette path lookup")
        _record(lookup.get("record"), evidence)
        evidence.check_origin(lookup.get("origin"))


def _record(value: object, evidence: SourceEvidence) -> None:
    record = read_object(value, "palette DataForge Record")
    _index(record.get("index"), 1 << 32, "DataForge Record")
    if not isinstance(record.get("guid"), str) or not record["guid"]:
        raise PackageError("A palette DataForge Record has no GUID.")
    evidence.check_origin(record.get("guidOrigin"))


def _bindings(
    value: object, nodes: list[Any], meshes: dict[str, dict[str, Any]],
    palette_ids: list[str], selection_count: int,
) -> None:
    rows = [
        read_object(item, "palette provenance binding")
        for item in read_array(value, "palette provenance bindings")
    ]
    ancestry = _ancestry(nodes) if any("inheritedFrom" in row for row in rows) else None
    seen: dict[int, tuple[int, int]] = {}
    used_payloads: set[int] = set()
    used_selections: set[int] = set()
    for row in rows:
        node = _index(row.get("node"), len(nodes), "node")
        payload = _index(row.get("payload"), len(palette_ids), "payload")
        selection = _index(row.get("selection"), selection_count, "selection")
        mesh_id = read_object(nodes[node], "palette binding node").get("mesh")
        mesh = meshes.get(mesh_id) if isinstance(mesh_id, str) else None
        if node in seen or mesh is None or mesh.get("palette_id") != palette_ids[payload]:
            raise PackageError("A palette binding is duplicated or does not match the node mesh.")
        if "inheritedFrom" in row:
            inherited = _index(row["inheritedFrom"], node, "inherited node")
            if seen.get(inherited) != (payload, selection):
                raise PackageError("An inherited palette binding does not match the supplying node.")
            if ancestry is None or not ancestry.contains(inherited, node):
                raise PackageError("The palette supplying node is not an ancestor of the bound node.")
        seen[node] = (payload, selection)
        used_payloads.add(payload)
        used_selections.add(selection)
    if len(used_payloads) != len(palette_ids) or len(used_selections) != selection_count:
        raise PackageError("The palette provenance contains an unreferenced payload or selection.")


class _Ancestry:
    def __init__(self, positions: list[int], ends: list[int]) -> None:
        self.positions = positions
        self.ends = ends

    def contains(self, ancestor: int, node: int) -> bool:
        return self.positions[ancestor] < self.positions[node] < self.ends[ancestor]


def _ancestry(nodes: list[Any]) -> _Ancestry:
    children: list[list[int]] = [[] for _ in nodes]
    roots = []
    for index, item in enumerate(nodes):
        parent = read_object(item, "palette binding node").get("parent")
        if parent is None:
            roots.append(index)
        else:
            children[_index(parent, index, "parent")].append(index)
    positions, ends = [0] * len(nodes), [0] * len(nodes)
    order = 0
    stack = [(index, False) for index in reversed(roots)]
    while stack:
        node, visited = stack.pop()
        if visited:
            ends[node] = order
            continue
        positions[node] = order
        order += 1
        stack.append((node, True))
        stack.extend((child, False) for child in reversed(children[node]))
    return _Ancestry(positions, ends)


def _index(value: object, limit: int, label: str) -> int:
    if type(value) is not int or not 0 <= value < limit:
        raise PackageError(f"A palette provenance {label} reference is outside its range.")
    return value
