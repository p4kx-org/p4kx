# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""The shader families, the build context and the surface a node graph is made of (ADR-0100)."""

from __future__ import annotations

from collections.abc import Callable, Mapping
from dataclasses import dataclass
from typing import NamedTuple

from .graph_spec import Ref
from .material_model import Colour, MaterialDesc, Palette

SURFACE_SHADERS = frozenset({"hardsurface", "illum", "layerblend", "layerblend_v2", "layer", "simple"})
GLASS_SHADERS = frozenset({"glasspbr"})
DECAL_SHADERS = frozenset({"meshdecal"})
HOLOGRAM_SHADERS = frozenset({"hologram", "hologramcig"})
NODRAW_SHADERS = frozenset({"nodraw", "nodraw_proxy", "proxy"})
MAX_BLENDED_LAYERS = 4
MASKED_LAYERS = 4
TINT_PALETTE_DECAL = "$TintPaletteDecal"
# The least total weight of the decal colours, so that a texel with no mask divides by a number.
DECAL_WEIGHT_FLOOR = 1e-6
# Generation-mask flag: the vertex colours tint the material.
PER_VERTEX_TINT = "PER_VERTEX_TINT"
# Name Blender's glTF importer gives the attribute of COLOR_0.
GLTF_COLOUR_ATTRIBUTE = "Color"
# Share of the Principled BSDF (reflection, refraction) in glass; the rest is the tinted transparent
# surface that EEVEE shows the scene through. 0.15 keeps clear glass as it looked when measured
# for ADR-0258: low enough to see the scene behind it, high enough to keep the reflections.
GLASS_BSDF_SHARE = 0.15
# Shader parameter: how far the palette's glass colour tints the glass (0 = clear, 1 = full).
TINT_STRENGTH = "GlassTintStrength"
MAX_PROPERTIES = 64
METAL_FROM = 0.15
METAL_TO = 0.5
SPECULAR_LEVEL_PER_F0 = 12.5
PARAM_PREFIX = "p4kx_param_"
NEUTRAL_SMOOTHNESS = 0.5
NEUTRAL_SMOOTHNESS_SHADERS = frozenset({"hardsurface", "layerblend_v2", "layer"})


@dataclass(frozen=True, slots=True)
class BuildContext:
    """What a graph needs besides the description: layer files, the mesh's palette, alphas.

    ``loadable`` says whether the file of a texture id loads; ``None`` means every texture does.
    A slot of any other texture counts as unwritten, so the graph uses the factors instead.
    """

    materials: Mapping[str, MaterialDesc]
    palette: Palette | None
    texture_alpha: Mapping[str, str]
    loadable: Callable[[str], bool] | None = None

    def usable(self, desc: MaterialDesc) -> MaterialDesc:
        """Returns ``desc`` without the texture slots that cannot be loaded."""
        return desc if self.loadable is None else desc.with_textures_only(self.loadable)


class Surface(NamedTuple):
    """The colour, roughness, metallic and normal of a surface.

    ``normal`` and ``level`` (the Specular IOR Level) are ``None`` without a map.
    """

    colour: Ref | Colour
    roughness: Ref | float
    normal: Ref | None
    metallic: Ref | float = 0.0
    level: Ref | None = None


def luminance(colour: Colour) -> float:
    """Returns the Rec. 709 luminance of a linear colour."""
    return 0.2126 * colour[0] + 0.7152 * colour[1] + 0.0722 * colour[2]


def metal_share(specular: Colour) -> float:
    """Returns how metallic a specular colour is: a ramp over its luminance from 0.15 to 0.5."""
    return min(max((luminance(specular) - METAL_FROM) / (METAL_TO - METAL_FROM), 0.0), 1.0)


def family_of(shader: str) -> str:
    """Returns ``surface``, ``glass``, ``decal``, ``hologram``, ``nodraw`` or ``unknown``."""
    name = shader.strip().lower()
    if name in SURFACE_SHADERS:
        return "surface"
    if name in GLASS_SHADERS:
        return "glass"
    if name in DECAL_SHADERS:
        return "decal"
    if name in HOLOGRAM_SHADERS:
        return "hologram"
    if name in NODRAW_SHADERS:
        return "nodraw"
    return "unknown"


def reads_vertex_colours(desc: MaterialDesc) -> bool:
    """Returns whether ``desc`` reads the vertex colours of its mesh.

    The package states it (``reads_vertex_colours``, the rule of ``Material::reads_vertex_colours``
    in p4kx-model, which also decides which primitives of the package meshes carry ``COLOR_0``). A package without
    the statement (before format 1.4) reads them for ``PER_VERTEX_TINT`` glass and for a layered
    surface without a blend or mask map, as the add-on did then.
    """
    if desc.stated_vertex_colours is not None:
        return desc.stated_vertex_colours
    family = family_of(desc.shader)
    if family == "glass":
        return PER_VERTEX_TINT in desc.gen_mask
    return family == "surface" and not (desc.slot("blend") or desc.slot("mask"))


def multiply(a: Colour, b: Colour) -> Colour:
    """Returns the channel-wise product of two colours."""
    return (a[0] * b[0], a[1] * b[1], a[2] * b[2])
