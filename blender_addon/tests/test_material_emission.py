# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Tests static emission from exact authored inputs, independent of dynamic Glow controls."""

from __future__ import annotations

from p4kx_importer.material_graph import BuildContext, build_graph
from p4kx_importer.material_model import parse_materials
from synthetic_materials import description, slot


def test_explicit_emittance_texture_is_tinted_once_and_keeps_surface_opacity() -> None:
    authored = _material()
    warnings: list[str] = []
    parsed = parse_materials([authored], warnings)
    graph = build_graph(parsed["x0001"], BuildContext(parsed, None, {"t0002": "none"}))
    bsdf = next(node for node in graph.nodes if node.kind == "ShaderNodeBsdfPrincipled")
    assert bsdf.inputs["Emission Strength"] == 2.0
    assert bsdf.inputs["Alpha"] == 0.45
    assert not warnings
    image = next(node for node in graph.nodes if node.texture == "t0002")
    multiply = next(node for node in graph.nodes if node.props.get("blend_type") == "MULTIPLY")
    assert multiply.inputs["B"] == (0.2, 0.4, 0.8)
    assert multiply.inputs["Factor"] == 1.0
    assert multiply.props.get("clamp_result") is not True
    assert graph.linked(bsdf.key, "Emission Color").key == multiply.key
    assert graph.linked(multiply.key, "A").key == image.key


def test_bad_or_unreadable_preview_retains_authored_flat_emission() -> None:
    authored = _material()
    authored["emission_source"]["preview"]["strength"] = float("inf")
    parsed = parse_materials([authored], [])
    graph = build_graph(parsed["x0001"], BuildContext(parsed, None, {}))
    bsdf = next(node for node in graph.nodes if node.kind == "ShaderNodeBsdfPrincipled")
    assert bsdf.inputs["Emission Color"] == (0.2, 0.4, 0.8)
    authored = _material()
    parsed = parse_materials([authored], [])
    graph = build_graph(parsed["x0001"], BuildContext(parsed, None, {}, loadable=lambda _: False))
    bsdf = next(node for node in graph.nodes if node.kind == "ShaderNodeBsdfPrincipled")
    assert bsdf.inputs["Emission Color"] == (0.2, 0.4, 0.8)


def _material() -> dict:
    return description(
        "x0001", "Illum", opacity=0.45, alpha_mode="blend", emissive=[0.2, 0.4, 0.8], glow=2.0,
        textures=[slot("Emittance", "other", "t0002")],
        emission_source={
            "source_complete": True,
            "emittance_requested": True,
            "preview": {"slot": "Emittance", "file": "other.dds", "tint": [0.2, 0.4, 0.8], "strength": 2.0},
        },
    )
