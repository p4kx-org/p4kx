# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Checks selected palette finish vectors against the ordinary importer graph."""

import json
import math
import unittest
from pathlib import Path

from p4kx_importer.graph_groups import LAYER_BLEND
from p4kx_importer.graph_spec import Ref
from p4kx_importer.material_graph import BuildContext, build_graph
from p4kx_importer.material_model import Palette, PaletteColours, parse_materials
from synthetic_materials import description, layer, slot


def finish_cases():
    fixture = Path(__file__).with_name("fixtures") / "palette_finish.json"
    return json.loads(fixture.read_text(encoding="utf-8"))["cases"]


def graph(case, detail=None, shader="HardSurface", carrier="alpha"):
    selected = "primary" if case["paletteByte"] is not None else None
    fields = {"gloss_mult": case["multiplier"]}
    if selected:
        fields["palette"] = selected
    samples = [slot("TexSlot6", "gloss", "t0001")] if carrier == "gloss" else [slot("TexSlot2", "normal", "t0001")]
    parameters = []
    if detail is not None:
        samples.append(slot("TexSlot7", "detail", "t0002"))
        parameters.append({"name": "DetailGloss", "value": str(detail)})
    raw = [description("x0001", shader, reads_vertex_colours=False,
        layers=[layer("Paint", "x0002", **fields)]),
        description("x0002", "Layer", textures=samples, parameters=parameters)]
    warnings = []
    materials = parse_materials(raw, warnings)
    assert not warnings, warnings
    entry = PaletteColours((1.0,) * 3, (0.04,) * 3, case["paletteByte"] or 0)
    palette = Palette("p0001", "synthetic", entry, entry, entry, (1.0,) * 3)
    return build_graph(materials["x0001"], BuildContext(materials, palette,
        {"t0001": "smoothness"}, ddna_pairs={"t0001": "t0003"} if carrier == "companion" else {}))


def evaluate(spec, value, smoothness, rgb=None, alpha=None, vertex_red=0.25):
    if not isinstance(value, Ref):
        return value
    node = spec.node(value.key)
    if node.kind == "ShaderNodeTexImage" and value.socket == "Alpha":
        assert node.texture == "t0001"
        return smoothness if alpha is None else alpha
    if node.kind == "ShaderNodeTexImage" and value.socket == "Color":
        assert node.texture in ("t0001", "t0003")
        return (*rgb, 1.0) if rgb is not None and node.texture == "t0001" else (smoothness,) * 3 + (1.0,)
    def source(socket):
        linked = spec.linked(node.key, socket)
        return evaluate(spec, linked if linked is not None else node.inputs[socket], smoothness, rgb, alpha, vertex_red)
    if node.kind == "ShaderNodeVertexColor":
        return (vertex_red, 0.5, 0.75, 1.0)
    if node.kind == "ShaderNodeGroup":
        assert node.group == LAYER_BLEND and value.socket == "Roughness"
        factor = source("Factor")
        return source("Roughness A") * (1.0 - factor) + source("Roughness B") * factor
    if node.kind == "ShaderNodeSeparateColor":
        assert value.socket == "Red"
        return source("Color")[0]
    if node.kind == "ShaderNodeRGBToBW":
        red, green, blue, _ = source("Color")
        return 0.2126 * red + 0.7152 * green + 0.0722 * blue
    assert node.kind == "ShaderNodeMath", node.kind
    left, right = source(0), source(1)
    if node.props["operation"] == "MULTIPLY":
        result = left * right
    elif node.props["operation"] == "SUBTRACT":
        result = left - right
    else:
        raise AssertionError(node.props["operation"])
    return min(max(result, 0.0), 1.0) if node.props["use_clamp"] else result


def roughness(spec, smoothness, rgb=None, alpha=None, vertex_red=0.25):
    bsdf = spec.node("bsdf")
    value = spec.linked(bsdf.key, "Roughness")
    return evaluate(spec, value if value is not None else bsdf.inputs["Roughness"], smoothness, rgb, alpha, vertex_red)


class PaletteFinishTest(unittest.TestCase):
    def test_shared_finish_vectors_and_disabled_detail(self):
        for case in finish_cases():
            for detail in (None, 0.0, 1.0):
                with self.subTest(case=case["name"], detail=detail):
                    actual = roughness(graph(case, detail), case["smoothness"])
                    self.assertTrue(math.isclose(actual, case["roughness"], abs_tol=1e-6), actual)

    def test_operator_discriminator_separates_the_three_alternatives(self):
        case = next(case for case in finish_cases() if case["name"] == "operator_discriminator")
        base_roughness = case["baseRoughness"]
        smoothness = case["smoothness"]
        palette = case["paletteByte"] / 255.0
        self.assertAlmostEqual(smoothness, 1.0 - base_roughness**2)

        actual = roughness(graph(case), smoothness)
        unpainted = roughness(graph({**case, "paletteByte": None}), smoothness)
        roughness_space = base_roughness * math.sqrt(1.0 - palette)
        sqrt_after_scaling = math.sqrt(1.0 - smoothness * palette)
        self.assertAlmostEqual(actual, case["roughness"])
        self.assertAlmostEqual(unpainted, 1.0 - smoothness)
        self.assertAlmostEqual(roughness_space, case["roughnessSpaceAlternative"])
        self.assertAlmostEqual(sqrt_after_scaling, case["sqrtAlternative"])
        for name, alternative in (
            ("unpainted", unpainted),
            ("roughness-space", roughness_space),
            ("square root after scaling", sqrt_after_scaling),
        ):
            with self.subTest(operator=name):
                self.assertGreater(abs(actual - alternative), 2.0 / 255.0)

    def test_ddna_rgb_gloss_and_lossless_companion_share_the_selected_finish(self):
        case = {"paletteByte": 204, "multiplier": 0.5, "smoothness": 0.75}
        for carrier in ("alpha", "gloss", "companion"):
            with self.subTest(carrier=carrier):
                self.assertAlmostEqual(roughness(graph(case, carrier=carrier), 0.75), 0.7)

    def test_selected_layer_gloss_reads_red_independently_of_alpha(self):
        case = {"paletteByte": 204, "multiplier": 0.5, "smoothness": 0.75}
        rgb = (37 / 255, 89 / 255, 201 / 255)
        for alpha in (0, 64, 128, 255):
            with self.subTest(alpha=alpha):
                actual = roughness(graph(case, carrier="gloss"), 0.75, rgb, alpha / 255)
                self.assertAlmostEqual(actual, 1 - rgb[0] * 0.5 * 0.8)

    def test_selected_alpha_reads_alpha_independently_of_non_grey_rgb(self):
        case = {"paletteByte": 204, "multiplier": 0.5, "smoothness": 0.75}
        rgb = (37 / 255, 89 / 255, 201 / 255)
        for alpha in (0, 64, 128, 255):
            with self.subTest(alpha=alpha):
                actual = roughness(graph(case), 0.75, rgb, alpha / 255)
                self.assertAlmostEqual(actual, 1 - alpha / 255 * 0.5 * 0.8)

    def test_own_selected_gloss_reads_red_and_unselected_keeps_luminance(self):
        rgb = (37 / 255, 89 / 255, 201 / 255)
        entry = PaletteColours((1.0,) * 3, (0.04,) * 3, 204)
        palette = Palette("p0001", "synthetic", entry, entry, entry, (1.0,) * 3)
        for selected in (False, True):
            raw = description("x0001", "HardSurface", palette="primary" if selected else None,
                              textures=[slot("TexSlot6", "gloss", "t0001")])
            materials = parse_materials([raw], [])
            spec = build_graph(materials["x0001"], BuildContext(materials, palette, {}))
            expected = 1 - rgb[0] * 0.8 if selected else 1 - sum(a * b for a, b in zip(rgb, (0.2126, 0.7152, 0.0722)))
            self.assertAlmostEqual(roughness(spec, 0.75, rgb, 112 / 255), expected)

    def test_two_layer_ao_with_zero_wear_selects_the_primary_finish(self):
        raw = [description("x0001", "HardSurface", reads_vertex_colours=True,
            gen_mask=["WEAR_LAYER", "VERTCOLORS"],
            parameters=[{"name": "WearBlendBase", "value": "0"}],
            textures=[slot("TexSlot1", "occlusion", "t0004")],
            layers=[layer("Paint", "x0002", palette="primary", gloss_mult=0.5),
                    layer("Wear", "x0003", palette="secondary")]),
            description("x0002", "Layer", textures=[slot("TexSlot6", "gloss", "t0001")]),
            description("x0003", "Layer")]
        materials = parse_materials(raw, [])
        entry = PaletteColours((1.0,) * 3, (0.04,) * 3, 204)
        palette = Palette("p0001", "synthetic", entry, entry, entry, (1.0,) * 3)
        self.assertIsNone(materials["x0001"].slot("diffuse"))
        spec = build_graph(materials["x0001"], BuildContext(materials, palette, {}))
        rgb = (37 / 255, 89 / 255, 201 / 255)
        for vertex_red in (0.0, 1.0):
            self.assertAlmostEqual(roughness(spec, 0.75, rgb, 112 / 255, vertex_red), 1 - rgb[0] * 0.5 * 0.8)

    def test_other_shader_keeps_its_sampled_finish(self):
        case = {"paletteByte": 51, "multiplier": 0.5, "smoothness": 0.75}
        self.assertAlmostEqual(roughness(graph(case, shader="Illum"), 0.75), 0.625)


if __name__ == "__main__":
    unittest.main()
