# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Tests of the description to node graph mapping and of the 1.1 reader, without Blender."""

from __future__ import annotations

from pathlib import Path

import pytest

from p4kx_importer import package
from p4kx_importer.graph_groups import LAYER_BLEND, WEAR, group_spec
from p4kx_importer.graph_spec import GraphSpec, NodeSpec, Ref
from p4kx_importer.material_graph import BuildContext, build_graph, family_of
from p4kx_importer.material_model import parse_materials, parse_palettes
from p4kx_importer.material_surface import GLASS_BSDF_SHARE
from p4kx_importer.material_pairing import index_of, pair_by_index
from synthetic_materials import description, entry, manifest_11, slot


def load() -> package.Package:
    return package.parse_manifest(manifest_11(), Path("."))


def graph_of(loaded: package.Package, material_id: str, palette: str | None = None) -> GraphSpec:
    context = BuildContext(
        loaded.material_descriptions,
        loaded.palettes.get(palette) if palette else None,
        {t.id: t.alpha for t in loaded.textures.values()},
    )
    return build_graph(loaded.material_descriptions[material_id], context)


def single(descriptions: list[dict], **context: object) -> GraphSpec:
    materials = parse_materials(descriptions, [])
    return build_graph(
        materials[descriptions[0]["id"]],
        BuildContext(materials, context.get("palette"), context.get("alpha", {})),  # type: ignore[arg-type]
    )


def test_the_reader_takes_materials_palettes_and_references() -> None:
    loaded = load()
    assert loaded.version == (1, 1)
    assert loaded.warnings == []
    assert set(loaded.material_descriptions) == {"x0001", "x0002", "x0003", "x0004"}
    assert loaded.palettes["p0001"].primary.glossiness == 200.0
    assert loaded.meshes["m0001"].palette_id == "p0001"
    assert loaded.meshes["m0001"].materials[0].material == "x0001"
    layered = loaded.material_descriptions["x0001"]
    assert layered.layers[1].gloss_mult == 0.8
    assert layered.number("WearBlendBase") == 0.5
    assert layered.slot("blend").texture == "t0001"  # type: ignore[union-attr]


def test_a_1_0_package_has_no_descriptions() -> None:
    from synthetic import manifest

    loaded = package.parse_manifest(manifest(), Path("."))
    assert loaded.material_descriptions == {}
    assert loaded.palettes == {}
    assert loaded.meshes["m0001"].materials[0].material is None


def test_malformed_descriptions_are_dropped_with_one_warning() -> None:
    warnings: list[str] = []
    result = parse_materials([{"id": "bad"}, 3, description("x0001", "Illum")], warnings)
    assert list(result) == ["x0001"]
    assert len(warnings) == 1
    assert parse_palettes([{"id": "p0001"}], warnings) == {}


def test_hostile_values_fall_back_to_neutral() -> None:
    raw = description("x0001", "Illum", diffuse=["a", 1, 2], opacity="x", alpha_mode="weird")
    material = parse_materials([raw], [])["x0001"]
    assert material.diffuse == (1.0, 1.0, 1.0)
    assert material.opacity == 1.0
    assert material.alpha_mode == "opaque"


def test_tex_mod_gives_tiling_and_offset() -> None:
    mod = {
        "name": "TexMod",
        "children": [],
        "attributes": [{"name": "TileU", "value": "4"}, {"name": "OffsetV", "value": "0.5"}],
    }
    raw = description("x0001", "Illum", textures=[slot("TexSlot1", "diffuse", "t0001", elements=[mod])])
    result = parse_materials([raw], [])["x0001"].textures[0]
    assert result.tiling == (4.0, 1.0)
    assert result.offset == (0.0, 0.5)


def test_shader_families() -> None:
    assert family_of("HardSurface") == "surface"
    assert family_of("illum") == "surface"
    assert family_of("GlassPBR") == "glass"
    assert family_of("MeshDecal") == "decal"
    assert family_of("Hologram") == "hologram"
    assert family_of("NoDraw") == "nodraw"
    assert family_of("SomethingNew") == "unknown"


def test_layered_tinted_material_blends_layers_by_the_blend_map() -> None:
    graph = graph_of(load(), "x0001", "p0001")
    assert graph.warnings == []
    assert graph.groups() == {LAYER_BLEND, WEAR}
    blends = [n for n in graph.nodes if n.group == LAYER_BLEND]
    assert len(blends) == 1
    blend = blends[0]
    assert graph.linked(blend.key, "Factor") is not None
    split = graph.linked(blend.key, "Factor")
    assert split is not None and split.socket == "Red"
    blend_images = [n for n in graph.nodes if n.texture == "t0001"]
    assert len(blend_images) == 1
    primary = (0.8 * 1.0, 0.1 * 0.9, 0.1 * 0.8)
    mixes = [n for n in graph.nodes if n.kind == "ShaderNodeMix" and n.props["blend_type"] == "MULTIPLY"]
    assert any(n.inputs.get("B") == pytest.approx(primary) for n in mixes)
    metallic = graph.linked("bsdf", "Metallic")
    assert metallic is not None and metallic.key == blend.key
    assert graph.properties["p4kx_param_WearBlendBase"] == "0.5"


def test_the_second_layer_takes_the_palette_gloss_and_gloss_mult() -> None:
    graph = graph_of(load(), "x0001", "p0001")
    blend = next(n for n in graph.nodes if n.group == LAYER_BLEND)
    assert blend.inputs["Roughness A"] == pytest.approx(1.0 - (100.0 / 255.0) * 0.8)
    assert graph.linked(blend.key, "Roughness B") is not None


def test_authored_zero_layer_gloss_multiplier_is_applied() -> None:
    raw = manifest_11()
    raw["materials"][0]["layers"][1]["gloss_mult"] = 0.0
    graph = graph_of(package.parse_manifest(raw, Path(".")), "x0001", "p0001")
    blend = next(n for n in graph.nodes if n.group == LAYER_BLEND)
    assert blend.inputs["Roughness A"] == 1.0


def test_layer_normal_and_smoothness_come_from_the_layer_texture() -> None:
    graph = graph_of(load(), "x0001", "p0001")
    kinds = graph.kinds()
    assert {"ShaderNodeNormalMap", "ShaderNodeNewGeometry", "ShaderNodeSeparateColor"} <= kinds
    blend = next(n for n in graph.nodes if n.group == LAYER_BLEND)
    assert graph.linked(blend.key, "Normal A") is not None
    assert graph.linked(blend.key, "Normal B") is not None
    assert graph.linked("bsdf", "Normal") == Ref(blend.key, "Normal")


def test_wear_uses_the_wear_map_and_parameter_for_layers_with_a_wear_tint() -> None:
    graph = graph_of(load(), "x0001", "p0001")
    wear = [n for n in graph.nodes if n.group == WEAR]
    assert len(wear) == 1
    assert wear[0].inputs["Wear Tint"] == (0.5, 0.4, 0.3)
    assert graph.linked(wear[0].key, "Wear") is not None


def test_without_a_palette_the_layers_keep_their_own_tint() -> None:
    graph = graph_of(load(), "x0001")
    blend = next(n for n in graph.nodes if n.group == LAYER_BLEND)
    assert graph.linked(blend.key, "Colour B") is not None or blend.inputs["Colour B"]


def test_without_a_blend_map_vertex_colours_blend() -> None:
    raw = [
        description("x0001", "HardSurface", layers=[{"name": "a", "material": "x0002"}, {"name": "b", "material": "x0002"}]),
        description("x0002", "Layer", diffuse=[0.3, 0.3, 0.3]),
    ]
    graph = single(raw)
    assert "ShaderNodeVertexColor" in graph.kinds()
    assert graph.groups() == {LAYER_BLEND}


def test_layers_beyond_the_blendable_ones_are_reported() -> None:
    layers = [{"name": str(i), "material": "x0002"} for i in range(7)]
    raw = [description("x0001", "HardSurface", layers=layers), description("x0002", "Layer")]
    graph = single(raw)
    assert any("layers" in w for w in graph.warnings)
    assert sum(n.group == LAYER_BLEND for n in graph.nodes) == 4


def test_unknown_shader_gets_the_basic_setup_and_a_warning() -> None:
    graph = graph_of(load(), "x0004")
    assert len(graph.warnings) == 1 and "WeirdShader" in graph.warnings[0]
    assert graph.linked("bsdf", "Base Color") is not None
    assert graph.groups() == set()


def test_palette_tint_replaces_the_diffuse_colour() -> None:
    palette = load().palettes["p0001"]
    graph = single([description("x0001", "Illum", palette="secondary", diffuse=[0.5, 0.5, 0.5])], palette=palette)
    assert graph.node("bsdf").inputs["Base Color"] == (0.1, 0.8, 0.1)
    assert graph.node("bsdf").inputs["Roughness"] == pytest.approx(1.0 - 100.0 / 255.0)


def test_diffuse_texture_is_multiplied_by_the_tint() -> None:
    palette = load().palettes["p0001"]
    raw = description("x0001", "Illum", palette="primary", textures=[slot("TexSlot1", "diffuse", "t0001")])
    graph = single([raw], palette=palette)
    colour = graph.linked("bsdf", "Base Color")
    assert colour is not None
    mix = graph.node(colour.key)
    assert mix.props["blend_type"] == "MULTIPLY"
    assert mix.inputs["B"] == (0.8, 0.1, 0.1)


def test_normal_map_smoothness_becomes_roughness() -> None:
    raw = description(
        "x0001", "Illum", textures=[slot("TexSlot2", "normal", "t0001"), slot("TexSlot4", "specular", "t0002")]
    )
    graph = single([raw], alpha={"t0001": "smoothness"})
    roughness = graph.linked("bsdf", "Roughness")
    assert roughness is not None
    assert graph.node(roughness.key).props["operation"] == "SUBTRACT"
    assert graph.linked("bsdf", "Specular IOR Level") is not None
    assert graph.linked("bsdf", "Normal") is not None


def test_alpha_modes() -> None:
    opaque = single([description("x0001", "Illum")])
    assert opaque.alpha_mode == "opaque" and opaque.linked("bsdf", "Alpha") is None
    blend = single([description("x0001", "Illum", alpha_mode="blend", opacity=0.4)])
    assert blend.alpha_mode == "blend" and blend.node("bsdf").inputs["Alpha"] == 0.4
    raw = description(
        "x0001", "Illum", alpha_mode="mask", alpha_test=0.5, textures=[slot("TexSlot1", "diffuse", "t0001")]
    )
    mask = single([raw], alpha={"t0001": "opacity"})
    alpha = mask.linked("bsdf", "Alpha")
    assert alpha is not None and mask.node(alpha.key).props["operation"] == "GREATER_THAN"


def test_a_hair_cap_is_blended_by_its_density_map() -> None:
    cap = description(
        "x0001",
        "HairPBR",
        alpha_mode="blend",
        gen_mask=["HAIR_CAP"],
        textures=[slot("TexSlot1", "mask", "t0001", file="a/m_hair_scalp.tif")],
    )
    graph = single([cap])
    assert graph.alpha_mode == "blend"
    alpha = graph.linked("bsdf", "Alpha")
    assert alpha is not None and graph.node(alpha.key).kind == "ShaderNodeRGBToBW"


def test_a_hair_cap_without_a_written_map_is_fully_transparent() -> None:
    for textures in ([], [slot("TexSlot1", "mask", None, file="a/m_hair_scalp.tif")]):
        cap = description("x0001", "HairPBR", alpha_mode="blend", gen_mask=["HAIR_CAP"], textures=textures)
        graph = single([cap])
        assert graph.alpha_mode == "blend"
        assert graph.node("bsdf").inputs["Alpha"] == 0.0


def test_glass_decal_hologram_and_nodraw() -> None:
    glass = single([description("x0001", "GlassPBR", palette="glass")], palette=load().palettes["p0001"])
    assert glass.node("bsdf").inputs["Transmission Weight"] == 1.0
    assert glass.node("bsdf").inputs["Base Color"] == (0.2, 0.3, 0.4)
    assert glass.alpha_mode == "blend"
    decal = single([description("x0001", "MeshDecal", textures=[slot("TexSlot1", "diffuse", None, file="$TintPaletteDecal")])], palette=load().palettes["p0001"])
    assert decal.alpha_mode == "blend"
    assert decal.node("bsdf").inputs["Base Color"] == (0.8, 0.1, 0.1)
    holo = single([description("x0001", "Hologram", glow=2.0)])
    assert holo.node("bsdf").inputs["Emission Strength"] == 2.0
    nodraw = single([description("x0001", "Illum", no_draw=True)])
    assert nodraw.kinds() == {"ShaderNodeBsdfTransparent", "ShaderNodeOutputMaterial"}


def test_per_vertex_tint_glass_takes_its_colour_from_the_vertex_colours() -> None:
    graph = single([description("x0001", "GlassPBR", gen_mask=["NORMAL_MAP", "PER_VERTEX_TINT"])])
    colours = graph.linked("bsdf", "Base Color")
    assert colours is not None
    assert graph.node(colours.key).kind == "ShaderNodeVertexColor"
    assert graph.node("bsdf").inputs["Transmission Weight"] == 1.0
    assert graph.alpha_mode == "blend"


def test_per_vertex_tint_glass_multiplies_the_vertex_colours_with_its_colour() -> None:
    raw = description("x0001", "GlassPBR", gen_mask=["PER_VERTEX_TINT"], diffuse=[0.5, 0.25, 1.0])
    graph = single([raw])
    tint = graph.linked("bsdf", "Base Color")
    assert tint is not None
    mix = graph.node(tint.key)
    assert (mix.kind, mix.props["blend_type"]) == ("ShaderNodeMix", "MULTIPLY")
    vertex = graph.linked(tint.key, "A")
    assert vertex is not None
    assert graph.node(vertex.key).kind == "ShaderNodeVertexColor"
    assert mix.inputs["B"] == (0.5, 0.25, 1.0)


def test_per_vertex_tint_glass_with_a_palette_colour_tints_that_colour() -> None:
    raw = description("x0001", "GlassPBR", gen_mask=["PER_VERTEX_TINT"], palette="glass")
    graph = single([raw], palette=load().palettes["p0001"])
    tint = graph.linked("bsdf", "Base Color")
    assert tint is not None
    assert graph.node(tint.key).inputs["B"] == (0.2, 0.3, 0.4)


def test_glass_without_per_vertex_tint_and_other_shaders_ignore_the_vertex_colours() -> None:
    clear = single([description("x0001", "GlassPBR", gen_mask=["NORMAL_MAP"])])
    assert clear.node("bsdf").inputs["Base Color"] == (1.0, 1.0, 1.0)
    illum = single([description("x0001", "Illum", gen_mask=["PER_VERTEX_TINT"])])
    assert "ShaderNodeVertexColor" not in illum.kinds()


def test_the_vertex_colour_node_names_the_attribute_the_gltf_import_writes() -> None:
    graph = single([description("x0001", "GlassPBR", gen_mask=["PER_VERTEX_TINT"])])
    colours = graph.linked("bsdf", "Base Color")
    assert colours is not None
    assert graph.node(colours.key).props == {"layer_name": "Color"}


def test_the_package_statement_decides_whether_glass_reads_the_vertex_colours() -> None:
    stated = single([description("x0001", "GlassPBR", gen_mask=["PER_VERTEX_TINT"], reads_vertex_colours=False)])
    assert "ShaderNodeVertexColor" not in stated.kinds()
    other = single([description("x0001", "GlassPBR", gen_mask=["NORMAL_MAP"], reads_vertex_colours=True)])
    assert "ShaderNodeVertexColor" in other.kinds()


def test_a_layered_surface_the_package_says_ignores_the_vertex_colours_is_not_blended() -> None:
    layers = [{"name": "a", "material": "x0002"}, {"name": "b", "material": "x0002"}]
    for stated, blended in ((False, False), (True, True)):
        raw = [
            description("x0001", "HardSurface", layers=layers, reads_vertex_colours=stated),
            description("x0002", "Layer", diffuse=[0.3, 0.3, 0.3]),
        ]
        graph = single(raw)
        assert ("ShaderNodeVertexColor" in graph.kinds()) is blended
        assert (LAYER_BLEND in graph.groups()) is blended


def test_a_package_before_1_4_keeps_the_earlier_vertex_colour_rule() -> None:
    glass = single([description("x0001", "GlassPBR", gen_mask=["PER_VERTEX_TINT"])])
    assert "ShaderNodeVertexColor" in glass.kinds()
    clear = single([description("x0001", "GlassPBR", gen_mask=["NORMAL_MAP"])])
    assert "ShaderNodeVertexColor" not in clear.kinds()
    layers = [{"name": "a", "material": "x0002"}, {"name": "b", "material": "x0002"}]
    surface = [description("x0001", "HardSurface", layers=layers), description("x0002", "Layer")]
    assert "ShaderNodeVertexColor" in single(surface).kinds()


def test_a_white_palette_glass_colour_with_the_flag_links_the_vertex_colours_alone() -> None:
    palette = parse_palettes(
        [
            {
                "id": "p0002",
                "name": "white",
                "primary": entry([0.8, 0.1, 0.1], 200.0),
                "secondary": entry([0.1, 0.8, 0.1], 100.0),
                "tertiary": entry([0.1, 0.1, 0.8], 50.0),
                "glass": [1.0, 1.0, 1.0],
            }
        ],
        [],
    )["p0002"]
    raw = description("x0001", "GlassPBR", gen_mask=["PER_VERTEX_TINT"], palette="glass")
    graph = single([raw], palette=palette)
    colours = graph.linked("bsdf", "Base Color")
    assert colours is not None
    assert graph.node(colours.key).kind == "ShaderNodeVertexColor"


@pytest.mark.parametrize(
    ("strength", "colour"),
    [(1.0, (0.2, 0.3, 0.4)), (0.5, (0.6, 0.65, 0.7)), (0.0, (1.0, 1.0, 1.0)), (4.0, (0.2, 0.3, 0.4))],
)
def test_glass_tint_strength_fades_the_palette_colour_towards_white(
    strength: float, colour: tuple[float, float, float]
) -> None:
    raw = description(
        "x0001",
        "GlassPBR",
        palette="glass",
        parameters=[{"name": "GlassTintStrength", "value": str(strength)}],
    )
    graph = single([raw], palette=load().palettes["p0001"])
    assert graph.node("bsdf").inputs["Base Color"] == pytest.approx(colour)


def glass_mix(graph: GraphSpec) -> str:
    """Returns the key of the Mix Shader that the output reads (no opacity fade)."""
    surface = graph.linked("output", "Surface")
    assert surface is not None and graph.node(surface.key).kind == "ShaderNodeMixShader"
    return surface.key


def tint_of(graph: GraphSpec, mix: str) -> NodeSpec:
    """Returns the Transparent BSDF node that tints the glass of ``mix``."""
    source = graph.linked(mix, 1)
    assert source is not None and graph.node(source.key).kind == "ShaderNodeBsdfTransparent"
    return graph.node(source.key)


def test_glass_is_a_tinted_transparent_surface_with_a_share_of_the_principled_bsdf() -> None:
    graph = single([description("x0001", "GlassPBR", diffuse=[0.5, 0.5, 0.5])])
    mix = glass_mix(graph)
    assert graph.node(mix).inputs["Fac"] == GLASS_BSDF_SHARE
    assert tint_of(graph, mix).inputs["Color"] == (0.5, 0.5, 0.5)
    assert graph.linked(mix, 2) == Ref("bsdf", "BSDF")
    assert graph.node("bsdf").inputs["Transmission Weight"] == 1.0
    assert "Alpha" not in graph.node("bsdf").inputs
    assert graph.alpha_mode == "blend"


def test_dark_glass_differs_from_grey_glass_only_in_its_tint() -> None:
    dark = single([description("x0001", "GlassPBR", diffuse=[0.0, 0.0, 0.0])])
    grey = single([description("x0001", "GlassPBR", diffuse=[0.16, 0.16, 0.16])])
    assert dark.node(glass_mix(dark)).inputs["Fac"] == grey.node(glass_mix(grey)).inputs["Fac"] == GLASS_BSDF_SHARE
    assert tint_of(dark, glass_mix(dark)).inputs["Color"] != tint_of(grey, glass_mix(grey)).inputs["Color"]


def test_palette_glass_colour_tints_the_transparent_surface() -> None:
    graph = single([description("x0001", "GlassPBR", palette="glass")], palette=load().palettes["p0001"])
    assert tint_of(graph, glass_mix(graph)).inputs["Color"] == (0.2, 0.3, 0.4)


def test_per_vertex_tint_glass_tints_the_transparent_surface_with_the_same_colour() -> None:
    graph = single([description("x0001", "GlassPBR", gen_mask=["PER_VERTEX_TINT"])])
    colours = graph.linked("bsdf", "Base Color")
    assert colours is not None
    assert graph.linked(tint_of(graph, glass_mix(graph)).key, "Color") == colours


def test_a_coloured_per_vertex_tint_feeds_one_multiply_mix_to_both_glass_colours() -> None:
    raw = description("x0001", "GlassPBR", gen_mask=["PER_VERTEX_TINT"], diffuse=[0.5, 0.25, 1.0])
    graph = single([raw])
    colours = graph.linked("bsdf", "Base Color")
    assert colours is not None and graph.node(colours.key).props["blend_type"] == "MULTIPLY"
    assert graph.node(colours.key).inputs["B"] == (0.5, 0.25, 1.0)
    assert graph.linked(tint_of(graph, glass_mix(graph)).key, "Color") == colours


def test_glass_opacity_fades_the_surface_to_clear() -> None:
    graph = single([description("x0001", "GlassPBR", opacity=0.5)])
    fade = graph.linked("output", "Surface")
    assert fade is not None and graph.node(fade.key).inputs["Fac"] == pytest.approx(0.5)
    clear = graph.linked(fade.key, 1)
    assert clear is not None and graph.node(clear.key).kind == "ShaderNodeBsdfTransparent"
    assert "Color" not in graph.node(clear.key).inputs
    glass = graph.linked(fade.key, 2)
    assert glass is not None and graph.node(glass.key).kind == "ShaderNodeMixShader"


NODRAW_KINDS = {"ShaderNodeBsdfTransparent", "ShaderNodeOutputMaterial"}


def test_a_decal_without_any_texture_draws_nothing() -> None:
    graph = single([description("x0001", "MeshDecal", alpha_mode="mask", alpha_test=0.01)])
    assert graph.kinds() == NODRAW_KINDS


def test_a_relief_decal_without_diffuse_opacity_draws_nothing() -> None:
    relief = description(
        "x0001",
        "Illum",
        gen_mask=["NORMAL_MAP", "DECAL"],
        alpha_mode="mask",
        textures=[slot("TexSlot1", "diffuse", "t0001")],
        parameters=[{"name": "DecalDiffuseOpacity", "value": "0"}],
    )
    assert single([relief]).kinds() == NODRAW_KINDS


def test_a_decal_with_diffuse_opacity_or_a_texture_is_still_drawn() -> None:
    painted = description(
        "x0001",
        "Illum",
        gen_mask=["DECAL"],
        textures=[slot("TexSlot1", "diffuse", "t0001")],
        parameters=[{"name": "DecalDiffuseOpacity", "value": "1"}],
    )
    assert single([painted]).kinds() != NODRAW_KINDS
    stencil = description("x0001", "MeshDecal", textures=[slot("TexSlot1", "diffuse", "t0001")])
    assert single([stencil]).kinds() != NODRAW_KINDS


def test_emission_counts_times_glow() -> None:
    lit = single([description("x0001", "Illum", emissive=[1.0, 0.5, 0.0], glow=3.0)])
    assert lit.node("bsdf").inputs["Emission Strength"] == 3.0
    dark = single([description("x0001", "Illum", emissive=[1.0, 0.5, 0.0], glow=0.0)])
    assert "Emission Strength" not in dark.node("bsdf").inputs


def test_two_sided_follows_the_description() -> None:
    assert single([description("x0001", "Illum", two_sided=True)]).two_sided is True
    assert single([description("x0001", "Illum")]).two_sided is False


def test_shared_groups_are_described_once_each() -> None:
    blend = group_spec(LAYER_BLEND)
    assert [name for name, _, _ in blend.inputs][-1] == "Factor"
    assert {n.kind for n in blend.graph.nodes} >= {"NodeGroupInput", "NodeGroupOutput", "ShaderNodeMix"}
    assert group_spec(WEAR).outputs == (("Colour", "NodeSocketColor"),)
    with pytest.raises(KeyError):
        group_spec("nope")


def test_pairing_prefers_the_p4kx_index_over_names() -> None:
    manifest = [(0, "Mat"), (1, "Mat"), (2, "Other")]
    created = [("Mat", 1), ("Mat.001", 0), ("Other", None)]
    assert pair_by_index(manifest, created) == {0: "Mat.001", 1: "Mat", 2: "Other"}


def test_pairing_falls_back_to_names_without_extras() -> None:
    manifest = [(0, "A"), (1, "B")]
    assert pair_by_index(manifest, [("B", None), ("A", None)]) == {0: "A", 1: "B"}


def test_index_property_values() -> None:
    assert index_of(3) == 3
    assert index_of(3.0) == 3
    assert index_of(True) is None
    assert index_of(-1) is None
    assert index_of("3") is None


def test_a_dark_diffuse_with_a_bright_specular_is_metal_with_the_specular_colour() -> None:
    bright = single([description("x0001", "Illum", diffuse=[0.02] * 3, specular=[0.9] * 3)])
    assert bright.node("bsdf").inputs["Base Color"] == (0.9, 0.9, 0.9)
    assert bright.node("bsdf").inputs["Metallic"] == 1.0
    plain = single([description("x0001", "Illum", diffuse=[0.02] * 3, specular=[0.04] * 3)])
    assert plain.node("bsdf").inputs["Base Color"] == (0.02, 0.02, 0.02)
    assert plain.node("bsdf").inputs["Metallic"] == 0.0
    assert "Specular Tint" not in plain.node("bsdf").inputs


def test_a_layer_with_a_bright_specular_blends_its_metal_share() -> None:
    raw = [
        description("x0001", "HardSurface", layers=[{"name": "a", "material": "x0002"}, {"name": "b", "material": "x0003"}]),
        description("x0002", "Layer", diffuse=[0.5] * 3, specular=[0.04] * 3),
        description("x0003", "Layer", diffuse=[0.02] * 3, specular=[0.9] * 3),
    ]
    graph = single(raw)
    blend = next(n for n in graph.nodes if n.group == LAYER_BLEND)
    assert blend.inputs["Metallic A"] == 0.0
    assert blend.inputs["Metallic B"] == 1.0
    assert blend.inputs["Colour B"] == (0.9, 0.9, 0.9)
    assert graph.linked("bsdf", "Metallic") == Ref(blend.key, "Metallic")


def test_a_partial_metal_share_mixes_diffuse_and_specular() -> None:
    graph = single([description("x0001", "Illum", diffuse=[0.1] * 3, specular=[0.325] * 3)])
    assert graph.node("bsdf").inputs["Metallic"] == pytest.approx(0.5, abs=0.01)
    colour = graph.linked("bsdf", "Base Color")
    assert colour is not None and graph.node(colour.key).kind == "ShaderNodeMix"


def test_a_specular_map_drives_metallic_colour_and_level_per_texel() -> None:
    raw = description(
        "x0001",
        "Illum",
        textures=[slot("TexSlot1", "diffuse", "t0001"), slot("TexSlot4", "specular", "t0002")],
    )
    graph = single([raw])
    metallic = graph.linked("bsdf", "Metallic")
    assert metallic is not None and graph.node(metallic.key).kind == "ShaderNodeMapRange"
    ramp = graph.node(metallic.key)
    assert ramp.inputs["From Min"] == 0.15 and ramp.inputs["From Max"] == 0.5
    level = graph.linked("bsdf", "Specular IOR Level")
    assert level is not None
    node = graph.node(level.key)
    assert node.props["operation"] == "MULTIPLY" and node.props["use_clamp"] is True
    assert node.inputs[1] == 12.5
    colour = graph.linked("bsdf", "Base Color")
    assert colour is not None and graph.node(colour.key).kind == "ShaderNodeMix"
    assert graph.linked(colour.key, "Factor") == metallic


def _specular_map_material(**values: object) -> GraphSpec:
    return single(
        [
            description(
                "x0001",
                "Illum",
                textures=[slot("TexSlot1", "diffuse", "t0001"), slot("TexSlot4", "specular", "t0002")],
                **values,
            )
        ]
    )


def test_a_specular_map_is_multiplied_by_the_specular_colour() -> None:
    graph = _specular_map_material(specular=[0.061] * 3)
    reflected = _multiply_nodes(graph)[0]
    assert reflected.inputs["B"] == (0.061,) * 3
    texture = graph.linked(reflected.key, "A")
    assert texture is not None and graph.node(texture.key).kind == "ShaderNodeTexImage"
    metallic = graph.linked("bsdf", "Metallic")
    assert metallic is not None
    ramp = graph.node(metallic.key)
    grey = graph.linked(ramp.key, "Value")
    assert grey is not None and graph.linked(grey.key, "Color") == Ref(reflected.key, "Result")
    colour = graph.linked("bsdf", "Base Color")
    assert colour is not None and graph.linked(colour.key, "B") == Ref(reflected.key, "Result")


def _multiply_nodes(graph: GraphSpec) -> list:
    return [n for n in graph.nodes if n.kind == "ShaderNodeMix" and n.props.get("blend_type") == "MULTIPLY"]


def test_a_white_specular_colour_feeds_the_map_straight_into_ramp_and_colour() -> None:
    graph = _specular_map_material(specular=[1.0] * 3)
    assert _multiply_nodes(graph) == []
    metallic = graph.linked("bsdf", "Metallic")
    assert metallic is not None
    grey = graph.linked(graph.node(metallic.key).key, "Value")
    assert grey is not None
    texture = graph.linked(grey.key, "Color")
    assert texture is not None and graph.node(texture.key).kind == "ShaderNodeTexImage"
    colour = graph.linked("bsdf", "Base Color")
    assert colour is not None and graph.linked(colour.key, "B") == texture


def test_a_map_without_a_specular_colour_uses_the_dielectric_default_and_is_no_metal() -> None:
    raw = description(
        "x0001",
        "Illum",
        textures=[slot("TexSlot1", "diffuse", "t0001"), slot("TexSlot4", "specular", "t0002")],
    )
    del raw["specular"]
    graph = single([raw])
    reflected = _multiply_nodes(graph)[0]
    assert reflected.inputs["B"] == (0.04, 0.04, 0.04)
    ramp = graph.node(graph.linked("bsdf", "Metallic").key)  # type: ignore[union-attr]
    grey = graph.linked(ramp.key, "Value")
    assert grey is not None and graph.linked(grey.key, "Color") == Ref(reflected.key, "Result")
    assert ramp.inputs["From Min"] == 0.15


def test_a_non_finite_colour_falls_back_to_the_default() -> None:
    graph = single([description("x0001", "Illum", specular=[float("nan"), float("inf"), 1.0])])
    assert graph.node("bsdf").inputs["Base Color"] == (1.0, 1.0, 1.0)
    assert graph.node("bsdf").inputs["Metallic"] == 0.0


def test_a_long_parameter_name_is_not_kept_as_a_property() -> None:
    long_name = "P" * 80
    raw = description(
        "x0001",
        "Illum",
        parameters=[{"name": long_name, "value": "1"}, {"name": "Short", "value": "2"}],
    )
    graph = single([raw])
    assert "p4kx_param_Short" in graph.properties
    assert not any(long_name in key for key in graph.properties)
    assert all(len(key.encode()) <= package.BLENDER_NAME_BYTES for key in graph.properties)
    assert len(graph.warnings) == 1


def test_roughness_without_a_gloss_map_is_neutral_for_layered_shaders_and_layers() -> None:
    hard = single([description("x0001", "HardSurface", shininess=255.0)])
    assert hard.node("bsdf").inputs["Roughness"] == 0.5
    illum = single([description("x0001", "Illum", shininess=255.0)])
    assert illum.node("bsdf").inputs["Roughness"] == 0.0


def test_the_smoothness_subtraction_is_clamped() -> None:
    graph = single(
        [description("x0001", "Illum", textures=[slot("TexSlot2", "normal", "t0001")])],
        alpha={"t0001": "smoothness"},
    )
    roughness = graph.linked("bsdf", "Roughness")
    assert roughness is not None and graph.node(roughness.key).props["use_clamp"] is True


def test_unused_alpha_and_wear_nodes_are_not_built() -> None:
    opaque = single([description("x0001", "Illum", textures=[slot("TexSlot3", "mask", "t0001")])])
    assert "ShaderNodeTexImage" not in opaque.kinds()
    layers = [{"name": "a", "material": "x0002"}, {"name": "b", "material": "x0002"}]
    raw = [
        description("x0001", "HardSurface", layers=layers, textures=[slot("TexSlot13", "wear", "t0001")]),
        description("x0002", "Layer"),
    ]
    assert "ShaderNodeTexImage" not in single(raw).kinds()
