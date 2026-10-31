# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Maps a material description to a node graph (ADR-0100), without Blender.

The mapping is a pure function of the description, the layer descriptions it names, the mesh's
palette and the alpha meaning of the textures, so all of it is tested without Blender.
"""

from __future__ import annotations

from .graph_spec import GraphBuilder, GraphSpec, Ref
from .material_glass import GlassSurface
from .material_layers import LayerBlending
from .package import BLENDER_NAME_BYTES
from .material_model import WHITE, Colour, Layer, MaterialDesc, PaletteColours, TextureSlot
from .material_surface import (
    DECAL_WEIGHT_FLOOR,
    MAX_PROPERTIES,
    METAL_FROM,
    METAL_TO,
    NEUTRAL_SMOOTHNESS,
    NEUTRAL_SMOOTHNESS_SHADERS,
    PARAM_PREFIX,
    SPECULAR_LEVEL_PER_F0,
    TINT_PALETTE_DECAL,
    BuildContext,
    Surface,
    family_of,
    luminance,
    metal_share,
    multiply,
)

__all__ = [
    "METAL_FROM",
    "METAL_TO",
    "SPECULAR_LEVEL_PER_F0",
    "BuildContext",
    "Surface",
    "build_graph",
    "family_of",
    "metal_share",
]


def build_graph(desc: MaterialDesc, context: BuildContext) -> GraphSpec:
    """Returns the node graph of ``desc`` as the game would draw it with ``context.palette``."""
    return _Maker(desc, context).build()


class _Maker(LayerBlending, GlassSurface):
    def __init__(self, desc: MaterialDesc, context: BuildContext) -> None:
        self.desc = context.usable(desc)
        self.context = context
        self.family = family_of(desc.shader)
        self.builder = GraphBuilder(self.family)
        self._images: dict[tuple[str, float, tuple[float, float], tuple[float, float]], str] = {}

    def build(self) -> GraphSpec:
        desc, spec = self.desc, self.builder.spec
        spec.two_sided = desc.two_sided
        spec.properties = self._properties()
        if desc.no_draw or self.family == "nodraw" or self._colourless_decal():
            return self._nodraw()
        if self.family == "unknown":
            spec.warnings.append(
                f"Shader {desc.shader!r} of material {desc.name or desc.id!r} is not known: "
                "a basic material is built."
            )
        surface = self._layered() if self.family == "surface" else self._base()
        self._principled(surface)
        return spec

    def _properties(self) -> dict[str, str]:
        properties = {
            "p4kx_material": self.desc.id,
            "p4kx_shader": self.desc.shader,
            "p4kx_family": self.family,
        }
        skipped = 0
        for name, value in list(self.desc.parameters.items())[:MAX_PROPERTIES]:
            key = PARAM_PREFIX + name
            if len(key.encode("utf-8")) > BLENDER_NAME_BYTES:
                skipped += 1
            else:
                properties[key] = value
        if skipped:
            self.builder.spec.warnings.append(
                f"Material {self.desc.name or self.desc.id!r} has {skipped} shader parameter(s) "
                "with a name too long to keep as a custom property."
            )
        return properties

    def _colourless_decal(self) -> bool:
        """Returns whether the material is a decal that draws no colour.

        That is a mesh decal without any texture slot, a stencil decal the package states as not
        drawn (its motif is a livery's emblem the game fills in, or its maps were not written)
        or whose map cannot be loaded here, a ``DECAL`` material whose ``DecalDiffuseOpacity`` is
        0 and which only adds relief.
        """
        desc = self.desc
        if self.family == "decal" and not desc.textures:
            return True
        if desc.stencil and not (
            desc.stencil.drawn and (desc.slot("stencil") or desc.slot("diffuse"))
        ):
            return True
        opacity = desc.number("DecalDiffuseOpacity")
        return "DECAL" in desc.gen_mask and opacity is not None and opacity <= 0.0

    def _nodraw(self) -> GraphSpec:
        bsdf = self.builder.add("ShaderNodeBsdfTransparent", key="bsdf")
        out = self.builder.add("ShaderNodeOutputMaterial", key="output")
        self.builder.link(Ref(bsdf, "BSDF"), out, "Surface")
        self.builder.spec.alpha_mode = "blend"
        return self.builder.spec

    def _decal_colours(self) -> tuple[Colour, Colour, Colour] | None:
        """Returns the decal colours the masks of the stencil map take, if the material takes them.

        That is a stencil decal the package states with the palette entry ``decal`` whose palette
        states decal colours; any other decal keeps its stencil colour.
        """
        palette = self.context.palette
        if self.desc.palette != "decal" or palette is None or not self.desc.slot("stencil"):
            return None
        return palette.decal

    def _decal_paint(self, colours: tuple[Colour, Colour, Colour]) -> Ref:
        """Returns the colour of a stencil map painted with ``colours``.

        The masks are layers, red below green below blue: each colour counts by its mask and by
        what the masks above leave of it, and the result is the weighted mean, so a texel that
        one mask covers partly takes none of the colours of the masks that are 0 there.
        """
        slot = self.desc.slot("stencil")
        assert slot is not None
        b = self.builder
        split = b.add("ShaderNodeSeparateColor")
        b.link(Ref(self._image(slot, 1.0), "Color"), split, "Color")
        mask_red, mask_green, mask_blue = (Ref(split, name) for name in ("Red", "Green", "Blue"))
        below_green = b.math("SUBTRACT", 1.0, mask_green)
        below_blue = b.math("SUBTRACT", 1.0, mask_blue)
        weight_red = b.math("MULTIPLY", b.math("MULTIPLY", mask_red, below_green), below_blue)
        weight_green = b.math("MULTIPLY", mask_green, below_blue)
        weights = (weight_red, weight_green, mask_blue)
        total = b.math("ADD", b.math("ADD", weight_red, weight_green), mask_blue)
        floor = b.math("MAXIMUM", total, DECAL_WEIGHT_FLOOR)
        weighted = [
            b.mix(colour, weight, 1.0, blend="MULTIPLY")
            for colour, weight in zip(colours, weights, strict=True)
        ]
        summed = b.mix(weighted[0], weighted[1], 1.0, blend="ADD")
        return b.mix(b.mix(summed, weighted[2], 1.0, blend="ADD"), floor, 1.0, blend="DIVIDE")


    def _entry(self) -> PaletteColours | None:
        palette = self.context.palette
        if palette is None:
            return None
        entry = palette.entry(self.desc.palette)
        if entry is None and self.family == "decal" and self._has_decal_placeholder():
            return palette.primary
        return entry

    def _has_decal_placeholder(self) -> bool:
        return any(s.file.startswith(TINT_PALETTE_DECAL) for s in self.desc.textures)

    def _base(self) -> Surface:
        entry = self._entry()
        stencil = self.desc.stencil
        factor = entry.tint if entry else (stencil and stencil.colour) or self.desc.diffuse
        if entry:
            smooth = entry.glossiness / 255.0
        elif self.desc.shader.lower() in NEUTRAL_SMOOTHNESS_SHADERS:
            smooth = NEUTRAL_SMOOTHNESS
        else:
            smooth = self.desc.shininess / 255.0
        specular = entry.specular if entry else self.desc.specular
        return self._surface(self.desc, factor, smooth, specular, 1.0, 1.0, entry is not None)

    def _surface(
        self,
        desc: MaterialDesc,
        factor: Colour,
        smooth: float,
        specular: Colour,
        gloss_mult: float,
        tiling: float,
        painted: bool,
    ) -> Surface:
        b = self.builder
        diffuse: Ref | Colour = factor
        if slot := desc.slot("diffuse"):
            texture = Ref(self._image(slot, tiling), "Color")
            diffuse = texture if factor == WHITE else b.mix(texture, factor, 1.0, blend="MULTIPLY")
        elif decal := self._decal_colours():
            diffuse = self._decal_paint(decal)
        if slot := desc.slot("occlusion"):
            diffuse = b.mix(diffuse, Ref(self._image(slot, tiling), "Color"), 1.0, blend="MULTIPLY")
        if slot := desc.slot("detail"):
            split = b.add("ShaderNodeSeparateColor")
            b.link(Ref(self._image(slot, tiling), "Color"), split, "Color")
            diffuse = b.mix(diffuse, Ref(split, "Red"), 0.5, blend="OVERLAY")
        colour, metallic, level = self._metal(desc, diffuse, specular, tiling, painted)
        normal: Ref | None = None
        smoothness: Ref | float | None = None
        if slot := desc.slot("normal"):
            image = self._image(slot, tiling)
            map_key = b.add("ShaderNodeNormalMap", props={"space": "TANGENT"})
            b.link(Ref(image, "Color"), map_key, "Color")
            normal = Ref(map_key, "Normal")
            if slot.texture and self.context.texture_alpha.get(slot.texture) == "smoothness":
                smoothness = Ref(image, "Alpha")
        if smoothness is None and (slot := desc.slot("gloss")):
            gloss = b.add("ShaderNodeRGBToBW")
            b.link(Ref(self._image(slot, tiling), "Color"), gloss, "Color")
            smoothness = Ref(gloss, "Val")
        if smoothness is None:
            roughness = min(max(1.0 - smooth * gloss_mult, 0.0), 1.0)
            return Surface(colour, roughness, normal, metallic, level)
        scaled = smoothness if gloss_mult == 1.0 else b.math("MULTIPLY", smoothness, gloss_mult)
        return Surface(colour, b.math("SUBTRACT", 1.0, scaled, clamp=True), normal, metallic, level)

    def _metal(
        self,
        desc: MaterialDesc,
        diffuse: Ref | Colour,
        specular: Colour,
        tiling: float,
        painted: bool,
    ) -> tuple[Ref | Colour, Ref | float, Ref | None]:
        b = self.builder
        if slot := desc.slot("specular"):
            texture = Ref(self._image(slot, tiling), "Color")
            if painted:
                grey = b.add("ShaderNodeRGBToBW")
                b.link(texture, grey, "Color")
                scale = SPECULAR_LEVEL_PER_F0 * luminance(specular)
                colour, share = self._by_share(diffuse, specular)
                return colour, share, b.math("MULTIPLY", Ref(grey, "Val"), scale, clamp=True)
            # Reflectance is the specular map times the material's Specular colour.
            reflected = texture if specular == WHITE else b.mix(texture, specular, 1.0, blend="MULTIPLY")
            grey = b.add("ShaderNodeRGBToBW")
            b.link(reflected, grey, "Color")
            texel = Ref(grey, "Val")
            ramp = b.add("ShaderNodeMapRange", props={"data_type": "FLOAT", "clamp": True})
            b.put(ramp, "Value", texel)
            b.put(ramp, "From Min", METAL_FROM)
            b.put(ramp, "From Max", METAL_TO)
            metallic = Ref(ramp, "Result")
            level = b.math("MULTIPLY", texel, SPECULAR_LEVEL_PER_F0, clamp=True)
            return b.mix(diffuse, reflected, metallic), metallic, level
        colour, share = self._by_share(diffuse, specular)
        return colour, share, None

    def _by_share(self, diffuse: Ref | Colour, specular: Colour) -> tuple[Ref | Colour, float]:
        share = metal_share(specular)
        if share == 0.0:
            return diffuse, 0.0
        if share == 1.0:
            return specular, 1.0
        return self.builder.mix(diffuse, specular, share), share

    def _layer_surface(self, layer: Layer, desc: MaterialDesc) -> Surface:
        palette = self.context.palette
        entry = palette.entry(layer.palette) if palette else None
        tint = layer.tint_colour
        if entry:
            factor = multiply(entry.tint, tint or WHITE)
            smooth = entry.glossiness / 255.0
            specular = entry.specular
        else:
            factor = tint or desc.diffuse
            smooth = NEUTRAL_SMOOTHNESS
            specular = desc.specular
        return self._surface(
            desc,
            factor,
            smooth,
            specular,
            layer.gloss_mult or 1.0,
            layer.uv_tiling or 1.0,
            entry is not None,
        )

    def _image(self, slot: TextureSlot, tiling: float) -> str:
        key = (slot.texture or "", tiling, slot.tiling, slot.offset)
        cached = self._images.get(key)
        if cached is not None:
            return cached
        b = self.builder
        image = b.add("ShaderNodeTexImage", texture=slot.texture)
        scale = (slot.tiling[0] * tiling, slot.tiling[1] * tiling)
        if scale != (1.0, 1.0) or slot.offset != (0.0, 0.0):
            coordinates = b.add("ShaderNodeTexCoord")
            mapping = b.add(
                "ShaderNodeMapping",
                props={"vector_type": "POINT"},
                inputs={"Location": (*slot.offset, 0.0), "Scale": (*scale, 1.0)},
            )
            b.link(Ref(coordinates, "UV"), mapping, "Vector")
            b.link(Ref(mapping, "Vector"), image, "Vector")
        self._images[key] = image
        return image

    def _alpha(self) -> Ref | float | None:
        desc, b = self.desc, self.builder
        mode = "blend" if self.family in ("glass", "decal", "hologram") else desc.alpha_mode
        self.builder.spec.alpha_mode = mode
        if mode not in ("mask", "blend"):
            return None
        source: Ref | None = None
        cap = desc.shader.lower() == "hairpbr" and "HAIR_CAP" in desc.gen_mask
        if slot := desc.slot("stencil"):
            # The red channel of a stencil map is the opacity of the motif; with decal colours,
            # the strongest of its three masks.
            split = b.add("ShaderNodeSeparateColor")
            b.link(Ref(self._image(slot, 1.0), "Color"), split, "Color")
            source = Ref(split, "Red")
            if self._decal_colours():
                strongest = b.math("MAXIMUM", source, Ref(split, "Green"))
                source = b.math("MAXIMUM", strongest, Ref(split, "Blue"))
        elif (slot := desc.slot("mask")) and (slot.texture or not cap):
            bw = b.add("ShaderNodeRGBToBW")
            b.link(Ref(self._image(slot, 1.0), "Color"), bw, "Color")
            source = Ref(bw, "Val")
        elif (
            (slot := desc.slot("diffuse"))
            and slot.texture
            and self.context.texture_alpha.get(slot.texture) == "opacity"
        ):
            source = Ref(self._image(slot, 1.0), "Alpha")
        if mode == "mask":
            return b.math("GREATER_THAN", source, desc.alpha_test) if source else None
        if mode != "blend":
            return None
        opacity = desc.opacity
        if self.family == "decal":
            if (extra := desc.number("DecalDiffuseOpacity")) is not None:
                opacity *= extra
            if desc.stencil:
                opacity *= desc.stencil.opacity
        if source is None:
            if cap:
                return 0.0
            return opacity if opacity < 1.0 else None
        return source if opacity == 1.0 else b.math("MULTIPLY", source, opacity)

    def _principled(self, surface: Surface) -> None:
        desc, b = self.desc, self.builder
        bsdf = b.add("ShaderNodeBsdfPrincipled", key="bsdf")
        out = b.add("ShaderNodeOutputMaterial", key="output")
        colour = surface.colour
        if self.family == "glass":
            colour = self._glass_base_colour(colour)
        b.put(bsdf, "Base Color", colour)
        b.put(bsdf, "Roughness", surface.roughness)
        if surface.normal is not None:
            b.link(surface.normal, bsdf, "Normal")
        b.put(bsdf, "Metallic", surface.metallic)
        if surface.level is not None:
            b.link(surface.level, bsdf, "Specular IOR Level")
        if self.family == "glass":
            b.spec.node(bsdf).inputs["Transmission Weight"] = 1.0
            b.spec.node(bsdf).inputs["IOR"] = 1.45
        emissive = desc.emissive
        if desc.glow > 0 and any(c > 0 for c in emissive):
            b.spec.node(bsdf).inputs["Emission Color"] = emissive
            b.spec.node(bsdf).inputs["Emission Strength"] = desc.glow
        if self.family == "hologram":
            b.put(bsdf, "Emission Color", colour)
            b.spec.node(bsdf).inputs["Emission Strength"] = max(desc.glow, 1.0)
        alpha = self._alpha()
        if self.family == "glass":
            b.link(self._glass_surface(bsdf, colour, alpha), out, "Surface")
            return
        b.link(Ref(bsdf, "BSDF"), out, "Surface")
        if alpha is not None:
            b.put(bsdf, "Alpha", alpha)
