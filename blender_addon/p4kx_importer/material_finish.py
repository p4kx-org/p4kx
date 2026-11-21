# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""Selects scalar gloss inputs for the authored HardSurface finish projection."""

from enum import Enum

from .graph_spec import GraphBuilder, Ref
from .material_model import MaterialDesc, Palette


class GlossChannel(Enum):
    """The component used by one explicit gloss image."""

    LUMINANCE = "luminance"
    RED = "red"


def gloss_channel(desc: MaterialDesc, palette: Palette | None) -> GlossChannel:
    """Returns red for a selected HardSurface finish and the generic channel otherwise."""
    if desc.shader.casefold() != "hardsurface" or palette is None:
        return GlossChannel.LUMINANCE
    if desc.layers:
        selected = any(palette.entry(layer.palette) is not None for layer in desc.layers)
    else:
        selected = palette.entry(desc.palette) is not None
    return GlossChannel.RED if selected else GlossChannel.LUMINANCE


def gloss_sample(builder: GraphBuilder, image: str, desc: MaterialDesc, palette: Palette | None) -> Ref:
    """Returns the selected scalar from an explicit gloss colour sample."""
    if gloss_channel(desc, palette) is GlossChannel.RED:
        scalar = builder.add("ShaderNodeSeparateColor")
        output = "Red"
    else:
        scalar = builder.add("ShaderNodeRGBToBW")
        output = "Val"
    builder.link(Ref(image, "Color"), scalar, "Color")
    return Ref(scalar, output)
