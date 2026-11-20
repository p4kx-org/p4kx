# Selected HardSurface palettes multiply sampled smoothness

Decided for #1162. Amends ADR-0080, ADR-0097, ADR-0100 and ADR-0147 for the explicitly
selected P4KX HardSurface palette plan. ADR-0207's detail-colour approximation stays separate.

## Context

The selected palette controls the finish of a painted surface. Dropping that input when a
smoothness image exists makes different selected finishes share the same roughness. The GLB
layer bake also needs the sampled finish of each contributing layer instead of a single scalar
fallback. The package already carries the original palette, layer multiplier and input textures.

Authenticated shader source gives a smoothness multiplication by the layer gloss and packed
palette gloss in the selected palette branch. Detail preprocessing and its generation flag are
separate source operations. Neither texture presence nor an authored material alone proves an
active runtime permutation, the CPU-to-GPU palette packing or the sampled runtime UV.

The direct isotropic GGX source consumer takes an alpha coordinate derived from smoothness;
the target glTF and supported Blender Principled conventions use perceptual roughness whose
square is alpha. This supports retaining the existing P4KX linear inverse coordinate. It does
not establish equality of every game lighting path, lobe, clamp or rendered material.

## Decision

For an explicitly selected P4KX HardSurface palette entry, let `S` be the selected linear
smoothness sample, `M` the authored `GlossMult` (default one), and `P` the selected palette
`glossiness / 255`, clamped to the unit interval. Both GLB and the ordinary Blender importer use
`R = clamp(1 - S * M * P, 0, 1)`. A layer without an entry uses `P = 1` for its sampled finish.
A material with no selected palette retains its existing unpainted sampled mapping.

Without a smoothness sample, the selected palette value remains the base smoothness, multiplied
by `M`, then subtracted from one with a final clamp. A layer without an entry keeps its neutral
scalar fallback. Multipliers are not clamped before this final result, matching the authored
controls in the ordinary importer. An unlayered selected palette has `M = 1`.

A selected HardSurface finish uses readable DDNA alpha first, then red from the first readable
authored explicit Gloss slot. The shader's scalar gloss helper reads `.x`; RGB luminance and
the independent alpha channel do not substitute for that scalar. The existing generic,
unselected RGB-Gloss projection stays separate and retains its luminance convention. The GLB layer bake retains
its existing admission, tiling, mask precedence and positional layer shares; its final green
channel blends the finished roughness of each contributing layer. Unused metalness stays
neutral, so the existing scalar metalness factor remains effective. Materials with their own
diffuse image retain their existing GLB admission and are not newly reinterpreted as layer bakes.

The new alpha consumer explicitly preserves and averages linear alpha during minification,
then samples alpha as a scalar. Existing RGB, base-colour and normal processing stays unchanged.
The output records the actual texture components, original contributing layer positions and
palette use through the existing provenance hooks, including original material and palette
sources. Equal image keys include the selected finish controls.

This is an authored P4KX approximation, not a runtime shader emulator. No new detail-gloss
operator, detail activation heuristic, palette-packing inference, or game upper-gloss clamp is
introduced. The DDNA alpha/companion contract describes P4KX's normalized decoded input; the
shader's scalar `.x` read does not establish an original DDS view, GPU swizzle or a DDNA-alpha
to-GPU binding. The authored palette normalization and multiplier remain P4KX model assumptions,
not proven CPU-to-GPU packing. Source proof, projection parity and rendered runtime comparison
are distinct gates.

## Verification

A shared synthetic vector file is read by the Rust finish/packed-channel tests and the pure
ordinary-importer graph test. It covers palette endpoints, multiplier zero and above one,
final clamping, absent palettes, unchanged other shaders and disabled detail. The existing
synthetic encrypted-Archive package fixture compares GLB output with the package's original
palette, multiplier, DDNA and explicit Gloss inputs; its Blender 5.3 signal probe checks the
corresponding ordinary imported graph. Additional layer tests check selected layer positions,
read authority, mask shares and sampled-versus-constant blending.

The alpha regression uses constant RGB with independently varying alpha and checks both modes.
Changing only the new alpha consumer to request RGB minification must fail this regression;
the original RGB mode must still produce opaque alpha. Non-grey RGB with independently varying
alpha also passes through the selected red and alpha consumers, the own selected packed route
and the ordinary graph. A two-layer AO wrapper with explicit zero wear checks primary-only
roughness and the original package layer shares. Installed source checks bind the game
Build and each selected original member before comparing supported LIVE and PTU sources.

The installed original-subset witness retains the decoded source UV bits in both ordinary
producers and compares their imported Roughness signals at four original triangle UV centroids.
A square orthographic render verifies each sample's projected position and unique foreground
identity. Four known linear scalar probes and two transparent background probes validate the
readback before comparison. The native Blender 5.3 check uses the retained
outputs from the installed LIVE subset.
Its private selection binds physical geometry, material, palette and texture Entries, including
split alpha headers. Verification receipts count actual checksum-to-EOF streams separately from
logical texture requests. Filtering draw ranges invalidates original first-use vertex coordinates;
this material witness requests no vertex-map sidecar. Its directly supplied scene declares no
Assembly palette-provenance claim. Native Blender 5.3 parity is a scalar-consumer check and does
not establish a runtime permutation, palette packing or rendered game appearance.

## Consequences

Selected painted surfaces can become rougher or smoother as their palette and layer controls
require. The package wire format and Catalog schema do not change. A layered GLB may embed one
additional roughness image, within the existing image/pixel accounting and failure reporting.
The documented GLB-versus-Blender layer-weight approximations remain visible.
