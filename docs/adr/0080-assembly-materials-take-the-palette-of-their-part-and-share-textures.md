# Assembly materials take the palette of their part and share textures

Decided for issue #127, which builds on ADR-0040 (the Assembly) and ADR-0077 (shared meshes).

## Decision

**Which palette a part has.** A part takes the tint palette its Entity's geometry names
(`SGeometryDataParams.Palette`, a `TintPaletteRef` whose `RootRecord` is a `TintPaletteTree`
Record). When the Entity names none, or the Record cannot be used, the part takes the
`DefaultPalette` its material library names. There is no fallback by name or family. A
`ChildPath` below the root palette is not supported yet: such a palette is reported and the part
falls back as for a missing one. Paint items attached to a Paint port, which choose a
sub-geometry and its palette, are chosen by the flags of their port (ADR-0208).

**Which materials take it.** A material takes the palette entry of its first `MatLayers` layer
with a `PaletteTint` of 1, 2 or 3 (primary, secondary, tertiary), and a `GlassPBR` material whose
generation mask names `USE_TINT_PALETTE` takes the glass colour. Palette colours are sRGB bytes
in the Record and are converted to linear values once, when the palette is read; no colour is
multiplied on sRGB bytes.

**How it is applied.** The palette replaces the diffuse colour, and for a painted material also
the specular colour and the shininess (the entry's glossiness, so the roughness follows the
paint and not the constant of the texture-gloss shaders). The glTF base colour factor carries the
tint over the base colour image, which therefore stays shared between liveries and is embedded
once, unless a specular map bakes the colours into the image. A texture that cannot be read keeps the factor as the plain colour.

**Sharing.** The key of ADR-0077 gains the identity of the part's palette: parts share a mesh and
its materials only when model, level, material library and palette are the same references. The
Assembly reads each distinct palette once, so the parts of one palette share it.

**Textures of the whole Assembly.** `write_scene_glb_with` makes the images of every part that
plans meshes of its own. An image is identified by its source files and the colours baked into it,
so equal images of different materials and parts are made once, and a repeated part adds no
loads. The images of a livery are shared with the other liveries unless a specular map bakes the
palette colours into the base and metallic-roughness images; then each palette has its own. Every texture is loaded at the one mip level of the options. Images are encoded before the
first byte of the file is written, a few at a time, within the budget of
`MAX_TEXTURE_IMAGES` and `MAX_TEXTURE_PIXELS`; a texture beyond it is reported like one that
cannot be read. A texture failure is listed once per material name, file and reason.

`shared_textures` counts the base colour, normal and metallic-roughness references of materials.
A texture slot added later (emissive, occlusion) must be counted there too.

## Consequences

`Assembly::build` resolves no palettes; `Assembly::build_with_palettes` does, through a
`PaletteProvider` such as `ForgePalettes`. A palette that cannot be resolved is a `palette` issue
of the first part that meets it and never drops a part. Parts without a palette keep the colours
of their library, as before.

Decal stencils (`$TintPaletteDecal`) and the layer textures of `MatLayers` materials are not
drawn: a hull material has no textures of its own, so it is a palette-coloured surface without
the layer detail until layered materials are written.
