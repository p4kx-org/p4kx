# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

"""A synthetic format 1.1 package with a layered, tinted material and an unknown shader."""

from __future__ import annotations

from typing import Any

from synthetic import manifest


def texture(number: int, colour_space: str, alpha: str) -> dict[str, Any]:
    """Returns the manifest entry of texture ``t000<number>``."""
    return {
        "id": f"t{number:04d}",
        "file": f"textures/t{number:04d}.png",
        "source": f"Textures/{number}.dds",
        "format": "png",
        "bit_depth": 8,
        "colour_space": colour_space,
        "alpha": alpha,
        "width": 1,
        "height": 1,
    }


def slot(name: str, usage: str, texture_id: str | None, **extra: Any) -> dict[str, Any]:
    """Returns a texture slot of a material description."""
    value: dict[str, Any] = {"slot": name, "file": f"{usage}.dds", "usage": usage}
    if texture_id:
        value["texture"] = texture_id
    value.update(extra)
    return value


def description(material_id: str, shader: str, **fields: Any) -> dict[str, Any]:
    """Returns a material description with neutral values and ``fields`` on top."""
    value: dict[str, Any] = {
        "id": material_id,
        "name": material_id,
        "library": "objects/test",
        "shader": shader,
        "gen_mask": [],
        "surface_type": "",
        "flags": 0,
        "two_sided": False,
        "no_draw": False,
        "diffuse": [1.0, 1.0, 1.0],
        "specular": [0.04, 0.04, 0.04],
        "emissive": [0.0, 0.0, 0.0],
        "shininess": 128.0,
        "opacity": 1.0,
        "alpha_test": 0.0,
        "glow": 0.0,
        "alpha_mode": "opaque",
        "textures": [],
        "layers": [],
        "parameters": [],
        "attributes": [],
        "elements": [],
    }
    value.update(fields)
    return value


def layer(name: str, material: str, **fields: Any) -> dict[str, Any]:
    """Returns a layer that names the description ``material``."""
    return {"name": name, "path": f"{name}.mtl", "material": material, "attributes": [], **fields}


def entry(tint: list[float], gloss: float) -> dict[str, Any]:
    """Returns a palette entry."""
    return {"tint": tint, "specular": [0.05, 0.05, 0.05], "glossiness": gloss}


def manifest_11() -> dict[str, Any]:
    """Returns a valid 1.1 manifest: m0001 has a layered tinted material, m0002 an unknown shader."""
    base = manifest()
    base["version"] = "1.1"
    base["textures"] = [
        texture(1, "linear", "none"),
        texture(2, "srgb", "none"),
        texture(3, "linear", "smoothness"),
        texture(4, "srgb", "none"),
        texture(5, "linear", "none"),
        texture(6, "linear", "none"),
    ]
    base["materials"] = [
        description(
            "x0001",
            "HardSurface",
            surface_type="metal_dense",
            palette="primary",
            textures=[slot("TexSlot12", "blend", "t0001"), slot("TexSlot13", "wear", "t0005")],
            layers=[
                layer("Primary", "x0002", palette="primary", tint_colour=[1.0, 0.9, 0.8]),
                layer(
                    "Secondary",
                    "x0003",
                    palette="secondary",
                    wear_tint=[0.5, 0.4, 0.3],
                    gloss_mult=0.8,
                    uv_tiling=2.0,
                ),
            ],
            parameters=[{"name": "WearBlendBase", "value": "0.5"}],
        ),
        description(
            "x0002",
            "Layer",
            textures=[slot("TexSlot1", "diffuse", "t0002"), slot("TexSlot2", "normal", "t0003")],
        ),
        description("x0003", "Layer", textures=[slot("TexSlot1", "diffuse", "t0004")]),
        description(
            "x0004",
            "WeirdShader",
            diffuse=[0.2, 0.4, 0.6],
            textures=[slot("TexSlot1", "diffuse", "t0002"), slot("TexSlot4", "specular", "t0006")],
        ),
    ]
    base["palettes"] = [
        {
            "id": "p0001",
            "name": "paint",
            "primary": entry([0.8, 0.1, 0.1], 200.0),
            "secondary": entry([0.1, 0.8, 0.1], 100.0),
            "tertiary": entry([0.1, 0.1, 0.8], 50.0),
            "glass": [0.2, 0.3, 0.4],
        }
    ]
    first, second = base["meshes"]
    first["palette_id"] = "p0001"
    first["materials"][0]["textures"] = {}
    first["materials"][0]["material"] = "x0001"
    second["materials"][0]["material"] = "x0004"
    second["palette_id"] = "p0001"
    second["materials"].append(
        {"index": 1, "name": "Box_mat2", "textures": {}, "material": "x0001"}
    )
    second["triangles"] = 2
    return base
