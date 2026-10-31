# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Material descriptions and palettes of a format 1.1 package, read without Blender.

The input is untrusted: a description that does not have the shape the schema promises is
dropped with a warning, a field of the wrong type falls back to the neutral value.
"""

from __future__ import annotations

import math
import re
from collections.abc import Callable
from dataclasses import dataclass, field, replace
from typing import Any

_MATERIAL_ID = re.compile(r"^x[0-9]{4,}$")
_PALETTE_ID = re.compile(r"^p[0-9]{4,}$")
_TEXTURE_ID = re.compile(r"^t[0-9]{4,}$")
PALETTE_ENTRIES = ("primary", "secondary", "tertiary", "glass")
USAGES = frozenset(
    {
        "diffuse",
        "normal",
        "specular",
        "mask",
        "blend",
        "detail",
        "wear",
        "occlusion",
        "displacement",
        "gloss",
        "stencil",
        "other",
    }
)
ALPHA_MODES = frozenset({"opaque", "mask", "blend"})

Colour = tuple[float, float, float]
WHITE: Colour = (1.0, 1.0, 1.0)
BLACK: Colour = (0.0, 0.0, 0.0)
DIELECTRIC: Colour = (0.04, 0.04, 0.04)


@dataclass(frozen=True, slots=True)
class TextureSlot:
    """One texture slot of a material: its usage, the texture id and the UV tiling and offset."""

    slot: str
    file: str
    usage: str
    texture: str | None
    tiling: tuple[float, float] = (1.0, 1.0)
    offset: tuple[float, float] = (0.0, 0.0)


@dataclass(frozen=True, slots=True)
class Layer:
    """A layer of a material: the layer file's description id and the layer's own inputs."""

    name: str
    material: str | None
    palette: str | None
    tint_colour: Colour | None
    wear_tint: Colour | None
    uv_tiling: float | None
    gloss_mult: float | None


@dataclass(frozen=True, slots=True)
class PaletteColours:
    """One entry of a palette: paint colour, specular colour and glossiness from 0 to 255."""

    tint: Colour
    specular: Colour
    glossiness: float


@dataclass(frozen=True, slots=True)
class Palette:
    """A tint palette: three paint entries and the glass colour."""

    id: str
    name: str
    primary: PaletteColours
    secondary: PaletteColours
    tertiary: PaletteColours
    glass: Colour

    def entry(self, name: str | None) -> PaletteColours | None:
        """Returns the paint entry called ``name``, or ``None`` for ``glass`` and unknown names."""
        if name == "primary":
            return self.primary
        if name == "secondary":
            return self.secondary
        if name == "tertiary":
            return self.tertiary
        return None


@dataclass(frozen=True, slots=True)
class Stencil:
    """How a stencil decal is drawn: whether it draws anything, its colour and opacity.

    ``colour`` is ``None`` for the diffuse colour. The package states colour and opacity only for a
    decal drawn by its stencil map, so they are applied as given.
    """

    drawn: bool
    colour: Colour | None
    opacity: float


@dataclass(frozen=True, slots=True)
class MaterialDesc:
    """The inputs of one game material, as the package describes them.

    ``stated_vertex_colours`` is the package's ``reads_vertex_colours`` (format 1.4), ``None`` in
    an older package; read it through ``material_surface.reads_vertex_colours``.
    ``layer_shares`` is the package's ``layer_shares`` (format 1.5), one share per layer, ``None``
    in an older package or when it does not match the layers.
    ``stencil`` is the package's ``stencil`` (format 1.6), present for a stencil decal only.
    """

    id: str
    name: str
    shader: str
    gen_mask: frozenset[str]
    surface_type: str
    two_sided: bool
    no_draw: bool
    diffuse: Colour
    specular: Colour
    emissive: Colour
    shininess: float
    opacity: float
    alpha_test: float
    glow: float
    alpha_mode: str
    palette: str | None
    textures: tuple[TextureSlot, ...] = ()
    layers: tuple[Layer, ...] = ()
    parameters: dict[str, str] = field(default_factory=dict)
    stated_vertex_colours: bool | None = None
    layer_shares: tuple[float, ...] | None = None
    stencil: Stencil | None = None

    def slot(self, usage: str) -> TextureSlot | None:
        """Returns the first texture slot of ``usage`` that has a written texture."""
        return next((s for s in self.textures if s.usage == usage and s.texture), None)

    def number(self, name: str) -> float | None:
        """Returns the shader parameter ``name`` as a finite number, or ``None``."""
        return _number(self.parameters.get(name))

    def colour_parameter(self, name: str) -> Colour | None:
        """Returns the shader parameter ``name`` as a colour (``r,g,b``), or ``None``.

        The value is the file's text: exactly three comma-separated finite numbers, else ``None``.
        """
        value = self.parameters.get(name)
        if value is None:
            return None
        return _colour([_number(part) for part in value.split(",")], None)

    def with_textures_only(self, loadable: Callable[[str], bool]) -> MaterialDesc:
        """Returns the description without the slots whose texture ``loadable`` refuses.

        A slot without a texture id (a placeholder) stays; it already reads as unwritten.
        """
        kept = tuple(s for s in self.textures if s.texture is None or loadable(s.texture))
        return self if len(kept) == len(self.textures) else replace(self, textures=kept)


def parse_materials(items: object, warnings: list[str]) -> dict[str, MaterialDesc]:
    """Reads the ``materials`` array; malformed entries are dropped with one warning."""
    materials: dict[str, MaterialDesc] = {}
    if not isinstance(items, list):
        return materials
    dropped = 0
    for item in items:
        material = _material(item)
        if material is None or material.id in materials:
            dropped += 1
        else:
            materials[material.id] = material
    if dropped:
        warnings.append(f"{dropped} material description(s) are malformed and were ignored.")
    return materials


def parse_palettes(items: object, warnings: list[str]) -> dict[str, Palette]:
    """Reads the ``palettes`` array; malformed entries are dropped with one warning."""
    palettes: dict[str, Palette] = {}
    if not isinstance(items, list):
        return palettes
    dropped = 0
    for item in items:
        palette = _palette(item)
        if palette is None or palette.id in palettes:
            dropped += 1
        else:
            palettes[palette.id] = palette
    if dropped:
        warnings.append(f"{dropped} palette(s) are malformed and were ignored.")
    return palettes


def _material(item: object) -> MaterialDesc | None:
    if not isinstance(item, dict):
        return None
    material_id = item.get("id")
    if not isinstance(material_id, str) or not _MATERIAL_ID.fullmatch(material_id):
        return None
    alpha_mode = item.get("alpha_mode")
    palette = item.get("palette")
    stated = item.get("reads_vertex_colours")
    layers = tuple(layer for layer in map(_layer, _list(item.get("layers"))) if layer)
    return MaterialDesc(
        id=material_id,
        name=_string(item.get("name")),
        shader=_string(item.get("shader")),
        gen_mask=frozenset(v for v in _list(item.get("gen_mask")) if isinstance(v, str)),
        surface_type=_string(item.get("surface_type")),
        two_sided=item.get("two_sided") is True,
        no_draw=item.get("no_draw") is True,
        diffuse=_colour(item.get("diffuse"), WHITE) or WHITE,
        specular=_colour(item.get("specular"), DIELECTRIC) or DIELECTRIC,
        emissive=_colour(item.get("emissive"), BLACK) or BLACK,
        shininess=_float(item.get("shininess"), 0.0),
        opacity=_float(item.get("opacity"), 1.0),
        alpha_test=_float(item.get("alpha_test"), 0.0),
        glow=_float(item.get("glow"), 0.0),
        alpha_mode=alpha_mode if alpha_mode in ALPHA_MODES else "opaque",
        palette=palette if palette in PALETTE_ENTRIES else None,
        textures=tuple(s for s in map(_slot, _list(item.get("textures"))) if s is not None),
        layers=layers,
        parameters=_parameters(item.get("parameters")),
        stated_vertex_colours=stated if isinstance(stated, bool) else None,
        layer_shares=_layer_shares(item.get("layer_shares"), len(layers)),
        stencil=_stencil(item.get("stencil")),
    )


def _stencil(value: object) -> Stencil | None:
    """Returns the stated stencil of a decal; a malformed opacity reads as 1."""
    if not isinstance(value, dict):
        return None
    opacity = min(max(_float(value.get("opacity"), 1.0), 0.0), 1.0)
    return Stencil(value.get("drawn") is not False, _colour(value.get("colour"), None), opacity)


def _layer_shares(value: object, count: int) -> tuple[float, ...] | None:
    """Returns the stated shares when there is one finite share of at least 0 per layer."""
    if not isinstance(value, list) or len(value) != count:
        return None
    shares = [_number(share) for share in value]
    if any(share is None or share < 0.0 for share in shares):
        return None
    return tuple(share for share in shares if share is not None)


def _slot(item: object) -> TextureSlot | None:
    if not isinstance(item, dict):
        return None
    usage = item.get("usage")
    texture = item.get("texture")
    tiling, offset = _tex_mod(_list(item.get("elements")))
    return TextureSlot(
        slot=_string(item.get("slot")),
        file=_string(item.get("file")),
        usage=usage if usage in USAGES else "other",
        texture=texture if isinstance(texture, str) and _TEXTURE_ID.fullmatch(texture) else None,
        tiling=tiling,
        offset=offset,
    )


def _tex_mod(elements: list[Any]) -> tuple[tuple[float, float], tuple[float, float]]:
    for element in elements:
        if not isinstance(element, dict) or element.get("name") != "TexMod":
            continue
        values = _parameters(element.get("attributes"))
        tiling = (_float(_number(values.get("TileU")), 1.0), _float(_number(values.get("TileV")), 1.0))
        offset = (
            _float(_number(values.get("OffsetU")), 0.0),
            _float(_number(values.get("OffsetV")), 0.0),
        )
        return tiling, offset
    return (1.0, 1.0), (0.0, 0.0)


def _layer(item: object) -> Layer | None:
    if not isinstance(item, dict):
        return None
    material = item.get("material")
    palette = item.get("palette")
    return Layer(
        name=_string(item.get("name")),
        material=material if isinstance(material, str) and _MATERIAL_ID.fullmatch(material) else None,
        palette=palette if palette in PALETTE_ENTRIES else None,
        tint_colour=_colour(item.get("tint_colour"), None),
        wear_tint=_colour(item.get("wear_tint"), None),
        uv_tiling=_optional_float(item.get("uv_tiling")),
        gloss_mult=_optional_float(item.get("gloss_mult")),
    )


def _palette(item: object) -> Palette | None:
    if not isinstance(item, dict):
        return None
    palette_id = item.get("id")
    if not isinstance(palette_id, str) or not _PALETTE_ID.fullmatch(palette_id):
        return None
    primary = _palette_colours(item.get("primary"))
    secondary = _palette_colours(item.get("secondary"))
    tertiary = _palette_colours(item.get("tertiary"))
    glass = _colour(item.get("glass"), None)
    if primary is None or secondary is None or tertiary is None or glass is None:
        return None
    return Palette(palette_id, _string(item.get("name")), primary, secondary, tertiary, glass)


def _palette_colours(item: object) -> PaletteColours | None:
    if not isinstance(item, dict):
        return None
    tint = _colour(item.get("tint"), None)
    specular = _colour(item.get("specular"), None)
    if tint is None or specular is None:
        return None
    return PaletteColours(tint, specular, min(max(_float(item.get("glossiness"), 0.0), 0.0), 255.0))


def _parameters(value: object) -> dict[str, str]:
    result: dict[str, str] = {}
    for entry in _list(value):
        if isinstance(entry, dict) and isinstance(entry.get("name"), str):
            if isinstance(entry.get("value"), str):
                result.setdefault(entry["name"], entry["value"])
    return result


def _list(value: object) -> list[Any]:
    return value if isinstance(value, list) else []


def _string(value: object) -> str:
    return value if isinstance(value, str) else ""


def _number(value: object) -> float | None:
    if isinstance(value, bool):
        return None
    if isinstance(value, (int, float)):
        number = float(value)
    elif isinstance(value, str):
        try:
            number = float(value.strip())
        except ValueError:
            return None
    else:
        return None
    return number if math.isfinite(number) else None


def _optional_float(value: object) -> float | None:
    return _number(value)


def _float(value: object, default: float) -> float:
    number = _number(value)
    return default if number is None else number


def _colour(value: object, default: Colour | None) -> Colour | None:
    if not isinstance(value, list) or len(value) != 3:
        return default
    numbers = [_number(v) for v in value]
    if any(n is None for n in numbers):
        return default
    return (numbers[0], numbers[1], numbers[2])  # type: ignore[return-value]
