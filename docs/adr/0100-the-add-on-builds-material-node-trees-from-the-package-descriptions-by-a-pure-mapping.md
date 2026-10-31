# The add-on builds material node trees from the package descriptions by a pure mapping

Decided for issue #160 (M13.4); it reads the descriptions of ADR-0092 in the add-on of ADR-0093.

Amended by ADR-0207: a layer that takes a palette entry or a tint other than white takes its metal
share from that entry's specular colour, and its specular map only sets the specular level; the
detail overlay uses the red channel of the packed detail map only; without a blend map the second
layer is blended by `(1 − vertex red) · WearBlendBase`; with a blend map the layers are blended by
the precedence of its channels. The mapping of the layers lives in `material_layers.py`.

Amended by #563: a material that blends its layers by neither a blend map, a mask map nor vertex
colours weights them by position as ADR-0097 does, folded into one blend per layer. The shares are
not computed twice: the package states them (`layer_shares`, format 1.5), computed by
`Material::layer_shares`, the code the GLB shades by. The first layer weighs `1 − WearBlendBase`,
the others `WearBlendBase` between them; a layer without a file has no weight. With no weight at
all the first layer takes everything when it exists, else the material keeps its own colours. The
add-on applies the same rule only to a package before 1.5.

Reach: the Rust rule makes a surface with two or more layers (counting the material's own diffuse
map) and no blend or mask map read the vertex colours, so those blend by the vertex colours and not
by the shares. The shares decide a single-layer surface, a material that is no surface, and a
material whose blend map cannot be loaded.

A texture whose file does not load (`BuildContext.loadable`, asked only for the textures a
material names) counts as an unwritten slot, in the material and in its layer files, so the graph
uses its factors.

Amended by ADR-0298: a stencil decal (`MeshDecal` with `STENCIL_MAP`) is stated by the package
(`stencil`, format 1.6), takes its alpha from the red channel of its `stencil` texture, its colour
from `stencil.colour` (else `diffuse`) and its opacity times `stencil.opacity`; a palette entry still
wins, and `StencilTintOverride` is the primary entry in the package's `palette`. A stencil decal
with neither a stencil nor a diffuse map is a Transparent BSDF.

## Decision

**Description in, graph description out, nodes last.** `material_graph.build_graph` maps a
material description, the descriptions of its layers, the mesh's palette and the alpha meaning of
the textures to a `GraphSpec`: plain data with node types, properties, input defaults and links.
`materials.py` is the only place that creates Blender nodes from it. The mapping needs no Blender,
so pytest covers it; the headless smoke test checks the nodes that come out. `package.py` reads
`materials`, `palettes`, `MeshMaterial.material` and `Mesh.palette_id` (`material_model.py`); a
malformed description or palette is dropped with one warning, never an error, and a 1.0 package
reads as having none. `SUPPORTED_MINOR` is 2 (the interiors of 1.2 are #161's; this work needs no new minor).

**Pairing.** Blender's glTF importer turns the `extras` `p4kx_index` of a material into a custom
property of the same name. A created material is paired with the mesh's manifest entry of that
index; materials without the property (a 1.0 package) are paired by name, as before. A paired
entry with a description gets the node tree of the description; one without, the basic setup of
ADR-0093 (its Specular IOR Level placeholder is replaced by the specular map rule below).

**Shader families.** The shader name, case folded, picks the family:

| Family | Shaders | Surface |
| --- | --- | --- |
| surface | `HardSurface`, `Illum`, `Layer`, `LayerBlend`, `LayerBlend_V2`, `Simple` | Principled BSDF; layers when the description has them |
| glass | `GlassPBR` | Principled with Transmission Weight 1, IOR 1.45, blended; the `glass` palette entry is the colour; with `PER_VERTEX_TINT` the colour is multiplied by the vertex colours (ADR-0253); the surface is 15 % of that Principled BSDF and 85 % a Transparent BSDF in the colour, faded by the description's alpha, so EEVEE shows the scene behind it tinted (ADR-0258, ADR-0282) |
| decal | `MeshDecal` | Principled, blended; a `$TintPaletteDecal` slot takes the palette's primary tint; `DecalDiffuseOpacity` scales the opacity; a stencil decal is cut out by its `stencil` map (ADR-0298) |
| hologram | `Hologram`, `HologramCIG` | Principled, the colour as emission (strength at least 1), blended |
| nodraw | `NoDraw`, `NoDraw_Proxy`, `Proxy`, or the `no_draw` flag | Transparent BSDF |
| unknown | anything else | the surface setup without layers, and a warning in the import report |

Every material also keeps `p4kx_material`, `p4kx_shader`, `p4kx_family` and its shader parameters
(`p4kx_param_<name>`, at most 64; a name whose key exceeds Blender's 63 bytes is skipped with one
warning) as custom properties, so a value the mapping does not use is
not lost for a script or a later add-on version.

**Texture roles.** By the description's `usage`: `diffuse` is the base colour (sRGB), `normal`
a Normal Map node (tangent space; the package's normal maps are already OpenGL, so nothing is
inverted), `specular` the metal share, base colour and specular level (below), `gloss` the smoothness, `occlusion`
multiplied into the colour, `detail` overlaid at 0.5, `mask` and the opacity alpha of the
diffuse map the alpha, `stencil` the alpha of a stencil decal (its red channel), `blend` and `wear`
the layer and wear factors. `displacement` and `other`
are not used. All but the diffuse map are linear (the package's `colour_space`). A `TexMod` of a
slot becomes a Mapping node (tiling as scale, offset as location). Smoothness is the alpha of a
normal map whose texture `alpha` is `smoothness` (the image's alpha mode is then channel packed
so the colour is not premultiplied) or a `gloss` map; roughness is `1 - smoothness`, the
subtraction clamped. Without a map the smoothness is `glossiness / 255` of the palette entry the
surface takes, else 0.5 for the shaders `HardSurface`, `LayerBlend_V2` and `Layer` and for every
layer, else `shininess / 255`, as the GLB does. `emissive` times `glow` becomes Emission when
`glow` is above 0.

**Metal and base colour** follow the GLB and the viewer (ADR-0097). Per surface the metal share is
`m`, a ramp over the luminance of the specular colour from 0.15 to 0.5 (the palette entry's
`specular` when the surface takes one, else the description's); Metallic is `m` and the base colour
is `mix(diffuse x tint, specular, m)`, the diffuse map being part of the diffuse term. Specular
Tint stays white. With a specular map the same is done per texel: Metallic is the map range of its
luminance over 0.15 to 0.5, the base colour mixes the diffuse term and the map by it, and the
Specular IOR Level is the luminance times 12.5, clamped. A dark paint over a bright specular is
therefore metal in the colour of the specular, not black. Without a description the basic setup
does the same for the glTF material's diffuse and specular maps. A missing `specular` is read as a
dielectric 0.04. Layers carry Metallic into the blend group like roughness; the Specular IOR Level
of a map comes from the foundation only.

**Palette tints.** The mesh's `palette_id` selects the palette; the material's `palette` the entry.
The entry's `tint` replaces the description's `diffuse` as the factor of the diffuse map (or is
the base colour without one), its `specular` feeds the metal share and its `glossiness` is the
smoothness. A mesh without a palette keeps the description's own colours.

**Layers.** A layer is drawn from the description its `material` names (its diffuse and normal
map, its own `diffuse` and shininess). Its colour factor is the entry named by its `palette`
times its `tint_colour`, else `tint_colour`, else its `diffuse`; `uv_tiling` scales its UVs and
`gloss_mult` its smoothness. If the base material has no diffuse map, the first layer is the
foundation of the surface; otherwise the base material is. Each further layer is blended over
the result by one channel of the base material's `blend` map (else `mask` map, else the mesh's
vertex colours): red for the first, green, blue and alpha. More than four blended layers are
cut off with a warning. The blend is the shared group **P4KX Layer Blend v1** (colour, roughness,
metallic and normal, the normal mixed and renormalised), so a normal is created only when a layer has
one. Nothing in the files fixes which channel belongs to which layer; this is the convention
used until a game-side reference says otherwise.

**Wear.** A layer with a `wear_tint` is multiplied by it where the wear factor is high, through
the shared group **P4KX Wear v1**. The group names carry a version: a changed interface gets a new name. The factor is the base material's `wear` map, times the
`WearBlendBase` parameter when there is one, or the parameter alone.

**Alpha.** `alpha_mode` `opaque` links no alpha. `mask` links the mask map (else the diffuse
alpha) through a greater-than of `alpha_test`. `blend` links the same source times `opacity`, or
the `opacity` itself; glass, decals and holograms are always blended. The material's render method
is `BLENDED` for blend and `DITHERED` otherwise, and back-face culling follows `two_sided`.

**Shared groups.** The two groups are described as data (`graph_groups.py`) and created in the
file on first use by name; every material instances the same group, so a hull with a hundred
materials has one of each.

## Consequences

New shader parameters keep arriving as custom properties and need no change to the mapping until
one is worth using; a new family is a name in `material_graph.py`, its graph, and a row above.
The layer channel convention is the first thing to revisit when a real ship is compared with the
game. Parallax and damage are not wired yet. The pure mapping is the place to follow the
colour derivation of the viewer and the GLB (ADR-0097).
