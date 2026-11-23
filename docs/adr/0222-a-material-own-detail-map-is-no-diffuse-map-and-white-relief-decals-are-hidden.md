# A material's own detail map is no diffuse map, and white relief decals are hidden

Decided for issue #757. It amends ADR-0212 (decals that draw no colour), ADR-0207 (the packed
detail map overlays through red only) and ADR-0100 (the add-on's surface mapping); ADR-0225 refines
the relief-decal rule. Evidence: the LIVE Archive, `AEGS_Hammerhead` and `AEGS_Gladius` as Blender
packages, compared against the owner's in-game screenshot of the Hammerhead.

## What the data says

- The `Metal_Flat_Light` material of the Hammerhead (`HardSurface`, `USE_DETAIL_MAPS`) names
  `aegs_ext_detail_01.tif` in `TexSlot1`. The file name ends in a number, so the suffix rule found
  no `_detail` and took the packed detail map for the diffuse map: its green and blue hold the
  detail normal, and multiplied into the grey hull they turned it purple.
- The `decal_pom` materials of the Hammerhead and the `pom`, `POM_Decal` and `detail_decals_pom`
  materials of the Gladius are `MeshDecal`s with `PARALLAX_OCCLUSION_MAPPING`, a displacement map
  (`_displ`), a white diffuse colour and `ETintModeType` 0. Their diffuse maps are white and grey
  (mean saturation below 0.01 on all of them: `aegs_exterior_pom_decal_diff`, `pom_decals_diff`,
  `line_decals_diff`, `crus_pom_diff`), the mask of the groove the displacement map cuts. Drawn as
  colour they paint white contour lines; in the game the panel lines are recesses.
- `Decal_POM` of the component master has the same white map but `ETintModeType` 6 and a tint map: a
  palette colours it, so it is not relief only. One `decal_pom` of the Hammerhead has a diffuse
  colour of 0.058: a colour of its own.
- The livery decal `Custom_decal` (`ship_showdown_decal.tif`) is in every Hammerhead material file:
  `aegs_hammerhead_exterior`, `_showdown`, `_camo_b` and `_fw2955_grey_black_red_camo` hold the same
  texture. The sub-geometries of the `Paint_Hammerhead_*` Paints swap only the tint palette and, for
  two of them, the material file. No palette carries a decal, no Paint a texture or a switch for
  this decal, and the hull number is not named in any Record we read.

## Decision

- **Texture suffix.** The suffix of a texture file name is its last `_` part that is not a plain
  number (`texture_suffix` in `p4kx-model`), so `aegs_ext_detail_01.tif` is `detail`. Both the GLB
  writer (no diffuse map) and the Blender package (`usage` `detail`, which the add-on overlays
  through red only, ADR-0207) use it.
- **Relief decals.** A material is a relief decal when it is a `MeshDecal` with
  `PARALLAX_OCCLUSION_MAPPING` in its generation mask and a displacement map, its diffuse colour is
  white (every channel from 0.99) and `ETintModeType` is 0 or missing (`Material::is_relief_decal`).
  It counts as a colourless decal, so `Material::hidden`, the GLB writer, the viewer and the add-on
  (`_relief_decal`) skip it. A decal with a colour of its own, a tint mode, no displacement map or
  no parallax mapping is drawn as before.
- **Livery decal and hull number.** Issue #775 keeps the authored presentation in 0.5.0. The
  export draws the selected material's authored emblem texture and the hull glyph from its
  original UVs. A paint name alone does not hide the emblem or select another glyph.

## Consequences

- The rule reads the material, not the pixels of the diffuse map, because the plan that hides a
  material runs before any texture is read. A white, untinted parallax decal whose map holds
  colour would be hidden too. None of the parallax decal maps of the two ships has colour.
- The relief itself (normal and displacement map) is not drawn, as for any colourless decal.
- The selected installed LIVE and PTU comparison checked physical material entries and related
  DataForge declarations. Both Builds retained the same emblem-bearing material set in scope.
  Empty observed decal ports and paint palette changes establish no separate visibility rule.
  The surveyed source also supplies no rule for selecting an alternative hull number. The
  authored presentation is the release contract. A comparison with the game's runtime choices
  would require independently verified selection evidence.
- Issue #1181 covers the light grey panel shading and roughness comparison.

## Factory Hammerhead bow comparison

For #1181, registration of two factory-paint LIVE captures binds the exposed bow plate to nose
mesh 2, subset 3, material slot 6. The registration residuals are 2.50 and 2.44 pixels. The
stripe decals provide geometric anchors. Use the adjacent unmarked plate for surface comparison.

The normal exporters produced the GLB and Blender package for the same physical subset on
LIVE 13220821 and PTU 13248297. Blender 5.3.0 build `02f0899d78e3` sampled the imported
Principled inputs at four original UVs with mip 2 inputs. The results agree across both Builds.

| Imported input | Paired result at the four UVs |
| --- | --- |
| Roughness | Maximum absolute difference 0.001878 |
| Linear base colour | Maximum absolute RGB component difference 0.000560 |
| Normal | Maximum absolute signed component difference 0.000000120 on the same tangent frame |
| IOR and metallic | 1.5 and 0 respectively in both imports |
| Specular IOR Level | Package 0.08033–0.13823, GLB 0.5 |

The package samples the original specular image using sRGB transfer, converts its linear RGB
to luminance, and applies the material multiplier and clamp. The measured conservative bound
for the admitted 512 × 512 mip is 0.007619–0.395728. At the repeat seam, filtering the encoded
RGB before decoding differs from filtering preconverted linear coefficients. The GLB import
uses Blender's default Specular IOR Level for this subset.

These measurements establish a material-input difference between the exporters. The game
captures provide display-space RGB under hangar lighting. Incident light, exposure, display
transform and active material parameters are still required to attribute the reported
appearance to a shader input. A matched linear material readback or calibrated runtime
comparison remains the evidence gate for a rendering change.

### Direct-sun game reference

The follow-up LIVE sequence shows the factory bow in direct Pyro sunlight. The local client
log and installed manifest agree on Change 13220821. In the two close views, held-out stripe
contours and the one-stripe-shift control confirm the plate immediately left of the stripes
as nose mesh 2, subset 3, slot 6. The snapshots use the same Vulkan renderer and reflection
settings.

Three unmarked regions were mapped between views using the plate's four orange triangle
markers. The table reports median luma in JPEG code values on a 0–255 scale, with 81 samples
per region. The nearby white stripe control uses the 80th percentile of the stripe window.
The sampled stripe windows have zero clipped channels in these three frames.

| View | Bare plate medians | Stripe control |
| --- | --- | ---: |
| 3 | 136.2, 137.2, 140.9 | 223.1 |
| 4 | 117.7, 119.7, 123.7 | 201.8 |
| 5 | 112.0, 112.0, 114.0 | 202.9 |

Views 3 and 4 report the same camera direction while the position and framing change. Both
the plate and the stripes become darker in view 4. In view 5 the stripe control is close to
view 4 and the plate values fall further. A nominal reprojection of one bare point from view 4 into view 5 changes the viewing angle
by 21.0°. In a 13 × 13 pixel window around that point, the median code value falls from
116.0 to 111.0. The sequence provides a display-space range for the plate. The stripe
reflectance and frame response are uncalibrated.
Attribution of the appearance difference still requires a matched material readback or a
calibrated lighting and display comparison.
