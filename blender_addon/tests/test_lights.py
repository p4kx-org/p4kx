# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Tests of the Interior nodes of a package and of the light mapping, without Blender."""

from __future__ import annotations

import copy
import math
from pathlib import Path
from typing import Any

import pytest

from p4kx_importer import lights, package
from p4kx_importer.light_model import Light
from p4kx_importer.package import PackageError, Quaternion, Vector3
from synthetic import interior_manifest, lit_manifest, manifest


def parse(data: dict[str, Any]) -> package.Package:
    return package.parse_manifest(data, Path("."))


def rotate(q: Quaternion, v: Vector3) -> Vector3:
    """Rotates ``v`` by the unit quaternion ``q`` given as ``(w, x, y, z)``."""
    w, x, y, z = q
    tx, ty, tz = (2 * (y * v[2] - z * v[1]), 2 * (z * v[0] - x * v[2]), 2 * (x * v[1] - y * v[0]))
    return (
        v[0] + w * tx + (y * tz - z * ty),
        v[1] + w * ty + (z * tx - x * tz),
        v[2] + w * tz + (x * ty - y * tx),
    )


def by_name(result: package.Package) -> dict[str, package.Node]:
    return {node.name: node for node in result.nodes}


def test_reads_containers_objects_and_lights() -> None:
    result = parse(lit_manifest())
    nodes = by_name(result)
    assert result.version == (1, 2)
    assert result.warnings == []
    assert [n.kind for n in result.nodes][4:] == [
        "container",
        "container",
        "object",
        "light",
        "light",
        "light",
    ]
    assert nodes["stool"].mesh == "m0002"
    assert nodes["room"].parent == 4
    lamp = nodes["lamp"].light
    assert lamp == Light("point", (1.0, 0.5, 0.25), 3.0, 6.0, None, None)
    beam = nodes["beam"].light
    assert beam is not None and beam.kind == "spot"
    assert beam.inner_cone == 0.0 and beam.outer_cone == pytest.approx(0.5235988)
    assert nodes["sun"].light is not None and nodes["sun"].light.range is None


def test_an_unknown_light_kind_is_skipped_with_a_warning() -> None:
    data = lit_manifest()
    data["nodes"][8]["light"]["kind"] = "area"
    result = parse(data)
    assert "beam" not in by_name(result)
    assert "lamp" in by_name(result)
    assert any("Lights of a kind" in warning for warning in result.warnings)


@pytest.mark.parametrize(
    ("path", "value"),
    [
        (("colour",), [2.0, 0.0, 0.0]),
        (("colour",), [0.5, 0.5]),
        (("intensity",), -1.0),
        (("intensity",), "bright"),
        (("range",), 0.0),
        (("range",), -2.0),
    ],
)
def test_a_malformed_light_is_refused(path: tuple[str, ...], value: object) -> None:
    data = lit_manifest()
    data["nodes"][7]["light"][path[0]] = value
    with pytest.raises(PackageError):
        parse(data)


@pytest.mark.parametrize(
    "cones",
    [(0.5, 0.2), (0.0, 2.0), (None, 0.5), (0.0, None), (-0.1, 0.5)],
)
def test_a_spot_light_needs_ordered_cones_within_a_quarter_turn(
    cones: tuple[float | None, float | None],
) -> None:
    data = lit_manifest()
    spot = data["nodes"][8]["light"]
    for key, value in zip(("inner_cone", "outer_cone"), cones):
        if value is None:
            spot.pop(key)
        else:
            spot[key] = value
    with pytest.raises(PackageError, match="spot light"):
        parse(data)


def test_a_cone_of_exactly_a_quarter_turn_in_single_precision_is_read() -> None:
    data = lit_manifest()
    data["nodes"][8]["light"]["outer_cone"] = 1.5707964
    assert by_name(parse(data))["beam"].light is not None


def test_a_light_node_without_a_light_is_refused() -> None:
    data = lit_manifest()
    del data["nodes"][7]["light"]
    with pytest.raises(PackageError, match="light must be an object"):
        parse(data)


def test_children_of_a_skipped_node_are_skipped_too() -> None:
    data = copy.deepcopy(lit_manifest())
    data["nodes"][5]["kind"] = "portal"
    result = parse(data)
    names = {node.name for node in result.nodes}
    assert {"room", "stool", "lamp", "beam", "sun"}.isdisjoint(names)
    assert "Interior" in names
    assert any("portal" in warning for warning in result.warnings)


def test_the_supported_minor_reads_without_a_warning_and_the_next_with_one() -> None:
    assert parse(manifest(version=f"1.{package.SUPPORTED_MINOR}")).warnings == []
    assert len(parse(manifest(version=f"1.{package.SUPPORTED_MINOR + 1}")).warnings) == 1


def test_point_light_settings() -> None:
    settings = lights.light_settings(Light("point", (1.0, 0.5, 0.25), 3.0, 6.0, None, None))
    assert settings.type == "POINT"
    assert settings.color == (1.0, 0.5, 0.25)
    assert settings.energy == pytest.approx(3.0 * lights.WATTS_PER_UNIT)
    assert settings.use_custom_distance and settings.cutoff_distance == 6.0
    assert settings.spot_size is None and settings.spot_blend is None


def test_spot_light_settings_use_the_full_cone_and_the_gltf_blend() -> None:
    light = Light("spot", (1.0, 1.0, 1.0), 2.0, None, math.radians(10), math.radians(30))
    settings = lights.light_settings(light)
    assert settings.type == "SPOT"
    assert settings.spot_size == pytest.approx(math.radians(60))
    assert settings.spot_blend == pytest.approx(1.0 - 10 / 30)
    assert not settings.use_custom_distance


def test_the_spot_cone_stays_within_blenders_limits() -> None:
    tiny = lights.light_settings(Light("spot", (1.0, 1.0, 1.0), 1.0, None, 0.0, 1e-6))
    wide = lights.light_settings(Light("spot", (1.0, 1.0, 1.0), 1.0, None, 0.0, math.pi / 2 + 0.3))
    assert tiny.spot_size == pytest.approx(lights.MIN_SPOT_SIZE)
    assert wide.spot_size == pytest.approx(lights.MAX_SPOT_SIZE)


def test_sun_settings_and_the_strength_multiplier() -> None:
    light = Light("directional", (0.9, 0.9, 1.0), 1.5, None, None, None)
    assert lights.light_settings(light).type == "SUN"
    assert lights.light_settings(light).energy == pytest.approx(1.5 * lights.SUN_STRENGTH_PER_UNIT)
    assert lights.light_settings(light, 2.0).energy == pytest.approx(3.0)
    assert lights.light_settings(light, -1.0).energy == 0.0


def test_a_light_keeps_its_direction_in_blenders_axes() -> None:
    turn: Quaternion = (0.0, math.sqrt(0.5), 0.0, math.sqrt(0.5))
    blender = lights.gltf_to_blender_light_quaternion(turn)
    shines = rotate(blender, (0.0, 0.0, -1.0))
    gltf_direction = (-1.0, 0.0, 0.0)
    expected = package.gltf_to_blender_vector(gltf_direction)
    assert shines == pytest.approx(expected, abs=1e-6)


def test_an_unturned_light_shines_down_the_gltf_minus_z_axis() -> None:
    blender = lights.gltf_to_blender_light_quaternion((0.0, 0.0, 0.0, 1.0))
    shines = rotate(blender, (0.0, 0.0, -1.0))
    assert shines == pytest.approx(package.gltf_to_blender_vector((0.0, 0.0, -1.0)), abs=1e-6)


def test_an_absurd_intensity_is_refused() -> None:
    data = lit_manifest()
    data["nodes"][7]["light"]["intensity"] = 2e9
    with pytest.raises(PackageError, match="intensity"):
        parse(data)


def test_children_of_a_light_node_are_skipped_with_a_warning() -> None:
    data = lit_manifest()
    child = {**data["nodes"][7], "kind": "container", "name": "under_lamp", "parent": 7}
    child.pop("light")
    data["nodes"].append(child)
    result = parse(data)
    assert "under_lamp" not in by_name(result)
    assert any("Children of light nodes" in warning for warning in result.warnings)


def test_a_mesh_on_a_light_node_is_ignored() -> None:
    data = lit_manifest()
    data["nodes"][7]["mesh"] = "m0001"
    assert by_name(parse(data))["lamp"].mesh is None


def test_the_energy_and_the_strength_are_capped() -> None:
    huge = Light("point", (1.0, 1.0, 1.0), 1e9, None, None, None)
    assert lights.light_settings(huge, 1000.0).energy == lights.MAX_ENERGY
    one = Light("point", (1.0, 1.0, 1.0), 1.0, None, None, None)
    assert lights.light_settings(one, 1e9).energy == pytest.approx(
        lights.WATTS_PER_UNIT * lights.MAX_STRENGTH
    )


def test_an_interior_on_its_own_reads_without_an_entity_node() -> None:
    loaded = parse(interior_manifest())

    assert loaded.warnings == []
    assert loaded.entity_name == "ship_interior"
    assert [node.kind for node in loaded.nodes] == ["container", "container", "object", "light"]
    assert loaded.nodes[0].parent is None
