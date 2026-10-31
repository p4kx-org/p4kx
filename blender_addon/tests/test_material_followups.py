# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Tests of layer shares without a blend source, missing textures and decal stencils."""

from __future__ import annotations

from collections.abc import Callable

import pytest

from p4kx_importer.graph_groups import LAYER_BLEND
from p4kx_importer.graph_spec import GraphSpec
from p4kx_importer.material_graph import BuildContext, build_graph
from p4kx_importer.material_model import parse_materials, parse_palettes
from synthetic_materials import description, entry, layer, slot

WEAR_BASE = "WearBlendBase"


def single(
    descriptions: list[dict],
    palette: dict | None = None,
    loadable: Callable[[str], bool] | None = None,
) -> GraphSpec:
    materials = parse_materials(descriptions, [])
    palettes = parse_palettes([palette], []) if palette else {}
    context = BuildContext(materials, palettes.get("p0001"), {}, loadable)
    return build_graph(materials[descriptions[0]["id"]], context)


def layered(
    wear: str | None,
    count: int,
    shares: list[float] | None = None,
    reads_vertex_colours: bool | None = False,
    blend_texture: str | None = None,
    loadable: Callable[[str], bool] | None = None,
) -> GraphSpec:
    """A HardSurface of ``count`` layers, as the package writer would state it.

    ``blend_texture`` gives it a blend map; ``reads_vertex_colours`` is the writer's statement.
    """
    layers = [
        layer(f"Layer{at}", f"x{at + 2:04d}", tint_colour=[0.1 * (at + 1)] * 3)
        for at in range(count)
    ]
    fields: dict = {"layers": layers}
    if wear is not None:
        fields["parameters"] = [{"name": WEAR_BASE, "value": wear}]
    if reads_vertex_colours is not None:
        fields["reads_vertex_colours"] = reads_vertex_colours
    if shares is not None:
        fields["layer_shares"] = shares
    if blend_texture is not None:
        fields["textures"] = [slot("TexSlot12", "blend", blend_texture)]
    raw = [
        description("x0001", "HardSurface", **fields),
        *(description(f"x{at + 2:04d}", "Layer") for at in range(count)),
    ]
    return single(raw, loadable=loadable)


def blends(graph: GraphSpec) -> list:
    return [node for node in graph.nodes if node.group == LAYER_BLEND]


def no_texture(_texture: str) -> bool:
    return False


def test_a_failed_blend_map_blends_the_layers_by_the_stated_shares() -> None:
    graph = layered("0.25", 2, [0.75, 0.25], blend_texture="t0001", loadable=no_texture)
    [blend] = blends(graph)
    assert blend.inputs["Factor"] == pytest.approx(0.25)
    assert blend.inputs["Colour A"] == pytest.approx((0.1, 0.1, 0.1))
    assert blend.inputs["Colour B"] == pytest.approx((0.2, 0.2, 0.2))


def test_stated_shares_are_divided_between_three_layers() -> None:
    graph = layered("0.6", 3, [0.4, 0.3, 0.3], blend_texture="t0001", loadable=no_texture)
    first, second = blends(graph)
    assert first.inputs["Factor"] == pytest.approx(0.3 / 0.7)
    assert second.inputs["Factor"] == pytest.approx(0.3)


def test_a_layer_without_a_stated_share_is_left_out() -> None:
    graph = layered("0", 2, [1.0, 0.0], blend_texture="t0001", loadable=no_texture)
    assert blends(graph) == []


def test_an_older_package_gets_the_same_shares_from_the_wear_parameter() -> None:
    graph = layered("0.3", 2, None, blend_texture="t0001", loadable=no_texture)
    [blend] = blends(graph)
    assert blend.inputs["Factor"] == pytest.approx(0.3)
    assert blends(layered("0", 2, None, blend_texture="t0001", loadable=no_texture)) == []


def test_a_single_layer_without_wear_weight_shows_that_layer() -> None:
    for shares in ([1.0], None):
        graph = layered("1", 1, shares)
        assert graph.node("bsdf").inputs["Base Color"] == pytest.approx((0.1, 0.1, 0.1))


def test_a_missing_first_layer_keeps_the_own_colours() -> None:
    raw = [
        description(
            "x0001",
            "HardSurface",
            diffuse=[0.7, 0.6, 0.5],
            layers=[layer("Layer0", "x9999")],
            reads_vertex_colours=False,
            layer_shares=[0.0],
        )
    ]
    graph = single(raw)
    assert graph.node("bsdf").inputs["Base Color"] == pytest.approx((0.7, 0.6, 0.5))


def test_stated_shares_that_do_not_match_the_layers_are_ignored() -> None:
    graph = layered("0.3", 2, [1.0], blend_texture="t0001", loadable=no_texture)
    [blend] = blends(graph)
    assert blend.inputs["Factor"] == pytest.approx(0.3)


def test_two_layers_without_a_blend_map_blend_by_the_vertex_colours() -> None:
    graph = layered("0.25", 2, [0.75, 0.25], reads_vertex_colours=True)
    [blend] = blends(graph)
    assert any(node.kind == "ShaderNodeVertexColor" for node in graph.nodes)
    factor = graph.linked(blend.key, "Factor")
    assert factor is not None and graph.node(factor.key).kind == "ShaderNodeMath"


def test_a_texture_that_does_not_load_falls_back_to_the_factors() -> None:
    raw = [
        description(
            "x0001",
            "Illum",
            diffuse=[0.2, 0.4, 0.6],
            textures=[slot("TexSlot1", "diffuse", "t0001"), slot("TexSlot2", "normal", "t0002")],
        )
    ]
    graph = single(raw, loadable=lambda texture: texture == "t0001")
    assert [n.texture for n in graph.nodes if n.kind == "ShaderNodeTexImage"] == ["t0001"]
    assert graph.linked("bsdf", "Normal") is None
    gone = single(raw, loadable=no_texture)
    assert not [n for n in gone.nodes if n.kind == "ShaderNodeTexImage"]
    assert gone.node("bsdf").inputs["Base Color"] == pytest.approx((0.2, 0.4, 0.6))


def test_only_the_textures_a_material_names_are_asked_for() -> None:
    asked: list[str] = []

    def loadable(texture: str) -> bool:
        asked.append(texture)
        return True

    raw = [description("x0001", "Illum", textures=[slot("TexSlot2", "normal", "t0002")])]
    single(raw, loadable=loadable)
    assert set(asked) == {"t0002"}


def test_every_texture_counts_as_loadable_without_a_predicate() -> None:
    raw = [description("x0001", "Illum", textures=[slot("TexSlot2", "normal", "t0002")])]
    assert single(raw).linked("bsdf", "Normal") is not None


def test_the_textures_of_a_layer_file_that_do_not_load_fall_back_too() -> None:
    raw = [
        description(
            "x0001", "HardSurface", layers=[layer("Layer0", "x0002", tint_colour=[0.3] * 3)]
        ),
        description("x0002", "Layer", textures=[slot("TexSlot1", "diffuse", "t0005")]),
    ]
    graph = single(raw, loadable=no_texture)
    assert not [n for n in graph.nodes if n.kind == "ShaderNodeTexImage"]
    assert graph.node("bsdf").inputs["Base Color"] == pytest.approx((0.3, 0.3, 0.3))


def decal(
    stencil: dict | None = None,
    textures: list[dict] | None = None,
    **fields: object,
) -> dict:
    """A decal that states ``stencil`` as the package does; by default with a stencil map."""
    maps = [slot("TexSlot7", "stencil", "t0007")] if textures is None else textures
    return description(
        "x0001",
        "MeshDecal",
        gen_mask=["STENCIL_MAP"],
        stencil={"drawn": True, "opacity": 1.0} if stencil is None else stencil,
        textures=maps,
        **fields,
    )


def alpha_texture(graph: GraphSpec) -> tuple[str, str]:
    """Returns the kind of the node and the output that feed the Alpha of the BSDF."""
    link = graph.linked("bsdf", "Alpha")
    assert link is not None
    return graph.node(link.key).kind, str(link.socket)


def test_a_stencil_decal_takes_its_alpha_from_the_red_of_the_stencil_map() -> None:
    graph = single([decal()])
    assert alpha_texture(graph) == ("ShaderNodeSeparateColor", "Red")
    assert len([n for n in graph.nodes if n.kind == "ShaderNodeTexImage"]) == 1


def test_the_stencil_map_wins_over_a_mask_map() -> None:
    mask = slot("TexSlot9", "mask", "t0009")
    graph = single([decal(textures=[slot("TexSlot7", "stencil", "t0007"), mask])])
    assert alpha_texture(graph) == ("ShaderNodeSeparateColor", "Red")


def test_a_stencil_decal_takes_its_stencil_colour() -> None:
    graph = single([decal({"drawn": True, "colour": [0.1, 0.2, 0.3], "opacity": 1.0})])
    assert graph.node("bsdf").inputs["Base Color"] == pytest.approx((0.1, 0.2, 0.3))


def test_a_stencil_decal_without_a_colour_takes_its_diffuse_colour() -> None:
    graph = single([decal(diffuse=[0.4, 0.5, 0.6])])
    assert graph.node("bsdf").inputs["Base Color"] == pytest.approx((0.4, 0.5, 0.6))


def test_a_palette_entry_wins_over_the_stencil_colour() -> None:
    palette = {
        "id": "p0001",
        "name": "p",
        "primary": entry([0.5, 0.5, 0.5], 100.0),
        "secondary": entry([0.5, 0.5, 0.5], 100.0),
        "tertiary": entry([0.5, 0.5, 0.5], 100.0),
        "glass": [1.0, 1.0, 1.0],
    }
    raw = decal({"drawn": True, "colour": [0.1, 0.2, 0.3], "opacity": 1.0}, palette="primary")
    graph = single([raw], palette)
    assert graph.node("bsdf").inputs["Base Color"] == pytest.approx((0.5, 0.5, 0.5))


def with_decal_colours(**fields: object) -> dict:
    """A palette that states the decal colours red, green and blue."""
    return {
        "id": "p0001",
        "name": "p",
        "primary": entry([0.5, 0.5, 0.5], 100.0),
        "secondary": entry([0.5, 0.5, 0.5], 100.0),
        "tertiary": entry([0.5, 0.5, 0.5], 100.0),
        "glass": [1.0, 1.0, 1.0],
        "decal": [[0.4, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 0.6]],
        **fields,
    }


def test_a_stencil_decal_that_takes_the_decal_colours_paints_its_masks_with_them() -> None:
    raw = decal({"drawn": True, "colour": [0.1, 0.2, 0.3], "opacity": 1.0}, palette="decal")
    graph = single([raw], with_decal_colours())
    base = graph.linked("bsdf", "Base Color")
    assert base is not None
    blue_mix = graph.node(base.key)
    assert blue_mix.kind == "ShaderNodeMix"
    assert blue_mix.inputs["B"] == pytest.approx((0.0, 0.0, 0.6))
    over = graph.linked(base.key, "A")
    assert over is not None
    green_mix = graph.node(over.key)
    assert green_mix.inputs["A"] == pytest.approx((0.4, 0.0, 0.0))
    assert green_mix.inputs["B"] == pytest.approx((0.0, 0.5, 0.0))
    for mix, channel in ((blue_mix, "Blue"), (green_mix, "Green")):
        factor = graph.linked(mix.key, "Factor")
        assert factor is not None and factor.socket == channel


def test_the_alpha_of_a_decal_with_decal_colours_is_the_strongest_of_its_masks() -> None:
    graph = single([decal(palette="decal")], with_decal_colours())
    link = graph.linked("bsdf", "Alpha")
    assert link is not None
    outer = graph.node(link.key)
    assert outer.kind == "ShaderNodeMath" and outer.props["operation"] == "MAXIMUM"
    inner = graph.linked(link.key, 0)
    assert inner is not None and graph.node(inner.key).props["operation"] == "MAXIMUM"
    assert len([n for n in graph.nodes if n.kind == "ShaderNodeTexImage"]) == 1


def test_a_decal_takes_its_stencil_colour_when_the_palette_states_no_decal_colours() -> None:
    palette = with_decal_colours()
    del palette["decal"]
    raw = decal({"drawn": True, "colour": [0.1, 0.2, 0.3], "opacity": 1.0}, palette="decal")
    graph = single([raw], palette)
    assert graph.node("bsdf").inputs["Base Color"] == pytest.approx((0.1, 0.2, 0.3))
    assert alpha_texture(graph) == ("ShaderNodeSeparateColor", "Red")
    assert single([raw]).node("bsdf").inputs["Base Color"] == pytest.approx((0.1, 0.2, 0.3))


def test_a_malformed_decal_of_a_palette_reads_as_none() -> None:
    for bad in ("red", [[0.1] * 3, [0.2] * 3], [[0.1] * 3, [0.2] * 3, "x"]):
        palettes = parse_palettes([with_decal_colours(decal=bad)], [])
        assert palettes["p0001"].decal is None
    assert parse_palettes([with_decal_colours()], [])["p0001"].decal is not None


def test_the_stencil_opacity_and_the_material_opacity_scale_the_alpha() -> None:
    graph = single([decal({"drawn": True, "opacity": 0.5}, opacity=0.8)])
    kind, _ = alpha_texture(graph)
    link = graph.linked("bsdf", "Alpha")
    assert kind == "ShaderNodeMath" and link is not None
    assert graph.node(link.key).inputs[1] == pytest.approx(0.4)


def test_a_stencil_opacity_of_zero_makes_the_alpha_zero() -> None:
    graph = single([decal({"drawn": True, "opacity": 0.0})])
    link = graph.linked("bsdf", "Alpha")
    assert link is not None
    assert graph.node(link.key).inputs[1] == 0.0


NOTHING_DRAWN = [
    "ShaderNodeBsdfTransparent",
    "ShaderNodeOutputMaterial",
]


def test_a_stencil_decal_the_package_states_as_not_drawn_draws_nothing() -> None:
    placeholder = [slot("TexSlot7", "other", None, file="$TintPaletteDecal")]
    graph = single([decal({"drawn": False, "opacity": 1.0}, placeholder)])
    assert [n.kind for n in graph.nodes] == NOTHING_DRAWN
    unwritten = single([decal({"drawn": False, "opacity": 1.0})])
    assert [n.kind for n in unwritten.nodes] == NOTHING_DRAWN


def test_a_stencil_map_that_cannot_be_loaded_here_draws_nothing() -> None:
    graph = single([decal()], loadable=no_texture)
    assert [n.kind for n in graph.nodes] == NOTHING_DRAWN


def test_a_stencil_decal_with_a_diffuse_map_is_drawn_by_it() -> None:
    kept = [slot("TexSlot1", "diffuse", "t0001")]
    graph = single([decal({"drawn": True, "opacity": 1.0}, kept)])
    assert graph.linked("bsdf", "Base Color") is not None
    assert graph.node("bsdf").kind == "ShaderNodeBsdfPrincipled"


def test_a_malformed_stencil_reads_as_neutral() -> None:
    raw = decal({"colour": "red", "opacity": "x"})
    stencil = parse_materials([raw], [])["x0001"].stencil
    assert stencil is not None
    assert stencil.drawn and stencil.colour is None and stencil.opacity == 1.0
    clamped = parse_materials([decal({"opacity": 7})], [])["x0001"].stencil
    assert clamped is not None and clamped.opacity == 1.0


def test_a_package_before_1_6_states_no_stencil() -> None:
    raw = description("x0001", "MeshDecal", gen_mask=["STENCIL_MAP"], textures=[slot("TexSlot7", "other", "t0007")])
    graph = single([raw])
    assert graph.node("bsdf").inputs["Base Color"] == pytest.approx((1.0, 1.0, 1.0))
