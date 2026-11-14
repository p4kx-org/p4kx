# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks references and byte ranges within the JSON of an embedded GLB."""

from __future__ import annotations

import re
from dataclasses import dataclass
from itertools import chain
from typing import Any

from .manifest_fields import PackageError, read_object

_COMPONENT_BYTES = {5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4}
_VECTOR_COMPONENTS = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}
_MATRIX_COLUMNS = {"MAT2": 2, "MAT3": 3, "MAT4": 4}
_INDEX_BYTES = {5121: 1, 5123: 2, 5125: 4}
_VERSION = re.compile(r"([0-9]+)\.([0-9]+)")


@dataclass(frozen=True, slots=True)
class BufferView:
    offset: int
    length: int
    stride: int | None


def validate_references(document: dict[str, Any], bin_length: int | None) -> tuple[int, int]:
    """Checks embedded asset links and returns declared buffer length and material count."""
    asset = read_object(document.get("asset"), "GLB asset")
    _asset_version(asset)
    buffers = _objects(document, "buffers")
    images = _objects(document, "images")
    for entry in chain(buffers, images):
        if "uri" in entry:
            raise PackageError("GLB refers to an external file in buffers or images.")
    data_length = _buffer_length(buffers, bin_length)
    views = _buffer_views(document, data_length, len(buffers))
    accessors = _objects(document, "accessors")
    for index, accessor in enumerate(accessors):
        _accessor(accessor, f"accessors[{index}]", views)
    for image in images:
        _reference(image.get("bufferView"), len(views), "image bufferView")
    material_count = len(_objects(document, "materials"))
    _scene_references(document, len(accessors), material_count)
    return data_length, material_count


def _asset_version(asset: dict[str, Any]) -> None:
    version = _version(asset.get("version"))
    if version is None:
        raise PackageError("Unsupported glTF asset version.")
    if "minVersion" not in asset:
        if version.group(1) != "2":
            raise PackageError("Unsupported glTF asset version.")
    else:
        minimum = _version(asset["minVersion"])
        if minimum is None or minimum.group() != "2.0":
            raise PackageError("Unsupported glTF asset minimum version.")


def _version(value: object) -> re.Match[str] | None:
    return _VERSION.fullmatch(value) if isinstance(value, str) else None

def _objects(parent: dict[str, Any], key: str) -> list[dict[str, Any]]:
    return [read_object(item, key) for item in _array(parent, key)]


def _array(parent: dict[str, Any], key: str) -> list[Any]:
    items = parent.get(key, [])
    if not isinstance(items, list):
        raise PackageError(f"GLB {key} must be an array.")
    return items


def _integer(value: object, name: str, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum:
        raise PackageError(f"GLB {name} must be an integer of at least {minimum}.")
    return value


def _reference(value: object, count: int, name: str) -> int:
    index = _integer(value, name)
    if index >= count:
        raise PackageError(f"GLB {name} index {index} is out of range.")
    return index


def _optional_reference(parent: dict[str, Any], key: str, count: int, name: str) -> None:
    if key in parent:
        _reference(parent[key], count, name)


def _buffer_length(buffers: list[dict[str, Any]], bin_length: int | None) -> int:
    if not buffers:
        if bin_length:
            raise PackageError("GLB BIN chunk has no declared buffer.")
        return 0
    if len(buffers) != 1:
        raise PackageError("GLB can embed only one buffer.")
    length = _integer(buffers[0].get("byteLength"), "buffer byteLength", 1)
    if bin_length is None:
        raise PackageError("GLB is missing the declared BIN chunk.")
    if not length <= bin_length <= length + 3:
        raise PackageError("GLB BIN chunk does not match the declared buffer byteLength.")
    return length


def _buffer_views(document: dict[str, Any], data_length: int, buffers: int) -> list[BufferView]:
    result: list[BufferView] = []
    for index, view in enumerate(_objects(document, "bufferViews")):
        where = f"bufferViews[{index}]"
        _reference(view.get("buffer"), buffers, f"{where}.buffer")
        start = _integer(view.get("byteOffset", 0), f"{where}.byteOffset")
        length = _integer(view.get("byteLength"), f"{where}.byteLength", 1)
        if start + length > data_length:
            raise PackageError(f"GLB {where} exceeds its buffer.")
        stride = None
        if "byteStride" in view:
            stride = _integer(view["byteStride"], f"{where}.byteStride", 4)
            if stride > 252 or stride % 4 != 0:
                raise PackageError(f"GLB {where}.byteStride is invalid.")
        result.append(BufferView(start, length, stride))
    return result


def _element_size(component_bytes: int, shape: object) -> tuple[int, int]:
    if isinstance(shape, str):
        if shape in _VECTOR_COMPONENTS:
            size = component_bytes * _VECTOR_COMPONENTS[shape]
            return size, size
        if shape in _MATRIX_COLUMNS:
            columns = _MATRIX_COLUMNS[shape]
            column_bytes = component_bytes * columns
            column_stride = (column_bytes + 3) // 4 * 4
            return column_stride * columns, column_stride * (columns - 1) + column_bytes
    raise PackageError("GLB accessor type is invalid.")


def _accessor(accessor: dict[str, Any], where: str, views: list[BufferView]) -> None:
    component = _integer(accessor.get("componentType"), f"{where}.componentType")
    if component not in _COMPONENT_BYTES:
        raise PackageError(f"GLB {where}.componentType is invalid.")
    alignment = _COMPONENT_BYTES[component]
    shape = accessor.get("type")
    stride, last = _element_size(alignment, shape)
    if isinstance(shape, str) and shape in _MATRIX_COLUMNS:
        alignment = 4
    count = _integer(accessor.get("count"), f"{where}.count", 1)
    if "bufferView" in accessor:
        _range(accessor, where, views, count, alignment, stride, last)
    elif "byteOffset" in accessor:
        raise PackageError(f"GLB {where}.byteOffset has no bufferView.")
    if "sparse" in accessor:
        _sparse(accessor["sparse"], where, views, count, alignment, stride, last)


def _range(
    entry: dict[str, Any],
    where: str,
    views: list[BufferView],
    count: int,
    alignment: int,
    stride: int,
    last: int,
    *,
    sparse: bool = False,
) -> None:
    view = views[_reference(entry.get("bufferView"), len(views), f"{where}.bufferView")]
    start = _integer(entry.get("byteOffset", 0), f"{where}.byteOffset")
    if sparse and view.stride is not None:
        raise PackageError(f"GLB {where} cannot use an interleaved bufferView.")
    effective_stride = view.stride if view.stride is not None else stride
    if start % alignment or (view.offset + start) % alignment:
        raise PackageError(f"GLB {where} is misaligned in its bufferView.")
    if effective_stride < stride or effective_stride % alignment:
        raise PackageError(f"GLB {where} has an invalid effective stride.")
    if start + (count - 1) * effective_stride + last > view.length:
        raise PackageError(f"GLB {where} exceeds its bufferView.")


def _sparse(
    value: object, where: str, views: list[BufferView], total: int, alignment: int, stride: int, last: int
) -> None:
    where += ".sparse"
    sparse = read_object(value, where)
    count = _integer(sparse.get("count"), f"{where}.count", 1)
    if count > total:
        raise PackageError(f"GLB {where}.count exceeds its accessor count.")
    indices = read_object(sparse.get("indices"), f"{where}.indices")
    component = _integer(indices.get("componentType"), f"{where}.indices.componentType")
    if component not in _INDEX_BYTES:
        raise PackageError(f"GLB {where}.indices.componentType is invalid.")
    size = _INDEX_BYTES[component]
    _range(indices, f"{where}.indices", views, count, size, size, size, sparse=True)
    values = read_object(sparse.get("values"), f"{where}.values")
    _range(values, f"{where}.values", views, count, alignment, stride, last, sparse=True)


def _scene_references(document: dict[str, Any], accessors: int, materials: int) -> None:
    scenes = _objects(document, "scenes")
    nodes = _objects(document, "nodes")
    meshes = _objects(document, "meshes")
    skins = _objects(document, "skins")
    _optional_reference(document, "scene", len(scenes), "default scene")
    for scene in scenes:
        for node in _array(scene, "nodes"):
            _reference(node, len(nodes), "scene node")
    for node in nodes:
        _optional_reference(node, "mesh", len(meshes), "node mesh")
        _optional_reference(node, "skin", len(skins), "node skin")
        for child in _array(node, "children"):
            _reference(child, len(nodes), "child node")
    for mesh in meshes:
        for primitive in _objects(mesh, "primitives"):
            _optional_reference(primitive, "material", materials, "primitive material")
            _optional_reference(primitive, "indices", accessors, "primitive indices accessor")
            attributes = read_object(primitive.get("attributes"), "GLB primitive attributes")
            for value in attributes.values():
                _reference(value, accessors, "primitive attribute accessor")
            for target in _objects(primitive, "targets"):
                for value in target.values():
                    _reference(value, accessors, "morph target accessor")
    for skin in skins:
        _optional_reference(skin, "inverseBindMatrices", accessors, "skin inverse bind accessor")
        _optional_reference(skin, "skeleton", len(nodes), "skin skeleton")
        for joint in _array(skin, "joints"):
            _reference(joint, len(nodes), "skin joint")
    for animation in _objects(document, "animations"):
        samplers = _objects(animation, "samplers")
        for sampler in samplers:
            _reference(sampler.get("input"), accessors, "animation input accessor")
            _reference(sampler.get("output"), accessors, "animation output accessor")
        for channel in _objects(animation, "channels"):
            _reference(channel.get("sampler"), len(samplers), "animation channel sampler")
            target = read_object(channel.get("target"), "GLB animation target")
            _optional_reference(target, "node", len(nodes), "animation target node")
