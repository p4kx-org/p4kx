# Capability-based media verification fixtures

Real-Archive canaries choose exact preferred public fixtures through the test-only selector in
`p4kx-testkit::media_fixture`. Assembly's shared adapter lives in
`crates/p4kx-assembly/tests/support/media_fixtures/`; production Entity lookup is unchanged.

## Selection contract

Each case declares a public family, required Entity kind, geometry and LOD policy, optional item
size, Interior or audio capability, and docked-vehicle expansion policy. The family table contains
public preferences already used by these tests and documented class prefixes; it contains no
private replacement names, Record GUIDs or asset paths from an installed test build.

Candidates retain the GUID, original Record path and physical Record-table index. Retrieval checks
all three against that exact position. A different Record with the same GUID cannot replace it.
Preferred matching compares complete class names with ASCII case folding. Duplicate preferred classes fail. A present
preference with an unreadable source, missing required geometry or incompatible capability fails;
it does not trigger fallback.

Only absent preferences permit deterministic alternatives inside the same documented family.
Eligible alternatives are ordered by physical source identity and checked against the same
capabilities. An absent compatible family produces an explicit absent outcome. Every
later registered case still runs after a failed or absent case; an entirely absent run fails.
A selected conversion failure remains a failed case. Selection and conversion are separate gates.

Canonical trees are read and inspected once. The resolver consumes the supplied tree and an
observer retains original typed dependency and required-file failures. Audio-only cases keep the
original trigger References and use `AudioIndex::resolve` without decoding the Entity again or
building an unrelated model graph. Dangling source links and malformed canonical data retain their
separate meanings.

Requested LOD and actual available geometry levels remain separate. The existing nearest-lower or
lowest-level policy is explicit in reported capabilities. Main model and Interior cases stop at
docked vehicles; other Assembly matrices retain their declared expansion policy. Existing minimum
parts, mesh sharing, placement, fallback-texture, palette, audio-resolution and media-location
thresholds remain active. Each requested texture mode has its own case outcome.

## Case scope

The shared adapter covers representative model, texture/palette, layer, follow-up, Interior,
Wwise-resolution and streamed/localised Assembly matrices. Exact regression fixtures for port
changes, attachment placement, cache identity or a particular authored Loadout keep their exact
source requirement; replacing them would change the invariant they test. Character and armour
cases with specific authored Loadouts also keep those requirements.

Ship, coarse-texture and Interior CLI package canaries reuse the shared capability selector.
Each CLI class/path argument is independently resolved to the chosen physical Record index, GUID
and path before invocation, and the returned report must identify the same Entity. These matrices
run every case and retain their existing geometry, material, sharing and size thresholds. CHF,
armour Loadout and animation requirements remain exact. Assembly-only results do not prove
package/importer behaviour; synthetic, real-Archive and Blender results are recorded separately.

## Privacy and evidence

Typed errors and complete identities remain available internally. Real-canary boundaries print
static failure categories, case labels, selection reasons, fulfilled capabilities and aggregate
counts. They do not format private source-bearing errors, replacement identities or game values.
Synthetic regression assertions may print only their authored synthetic fixtures.

The synthetic selector/report suite covers deterministic fallback, ambiguity, absent scope,
present incompatible preference, later-case execution, selected-conversion failure and safe error
formatting. Adapter tests independently cover physical GUID aliases, canonical/field errors,
missing declared geometry, dependency target identity and actual XML read/parse failures.

Actual LIVE/PTU canaries are opt-in tests configured with `P4KX_DATA_P4K` and the normal key helper.
Run them with `TMP`, `TEMP` and an isolated `P4KX_FOLDERS_ROOT` under the authorised agent task
folder. A synthetic pass is not actual Archive coverage. Record source build, channel, completed,
absent and failed case counts with any remaining failure categories.

Affected consumer outputs also require the source importer in an isolated Blender 5.2 process:
record the importer/source and package hashes, hierarchy, bounds, rig, images and material
assignments, plus fixed-camera renders for changed visible behaviour. Import success alone does
not establish original-source completeness or in-game equivalence. The offered interactive
Blender session remains unchanged.

## Rust-exported synthetic packages in Blender

`p4kx-testkit` writes synthetic source Archives for end-to-end exporter/importer checks.
Build the CLI from the same checkout as the importer. In a dedicated PowerShell session, choose
an absolute scratch root outside the repository and the game installation:

```powershell
$verificationRoot = 'D:\Coding\Agents\Temp\codex\p4kx-blender-verification'
$fixtures = Join-Path $verificationRoot 'fixtures'
$packages = Join-Path $verificationRoot 'packages'
$env:CARGO_TARGET_DIR = Join-Path $verificationRoot 'build'
$env:TMP = Join-Path $verificationRoot 'tmp'
$env:TEMP = $env:TMP
$env:P4KX_FOLDERS_ROOT = Join-Path $verificationRoot 'profile'
$env:P4KX_LAUNCHER_DIR = Join-Path $verificationRoot 'launcher'
$env:P4KX_GAME_CHECK = 'off'
Remove-Item Env:P4KX_ARCHIVE_KEY -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $env:TMP, $env:P4KX_LAUNCHER_DIR, $packages | Out-Null
cargo run --locked -p p4kx-testkit --example blender_consumer_fixture -- $fixtures
$env:P4KX_SOURCE_REVISION = (git rev-parse HEAD).Trim()
cargo build --locked -p p4kx --bin p4kx
$p4kx = Join-Path $env:CARGO_TARGET_DIR 'debug/p4kx.exe'
& $p4kx --json --key-file "$fixtures/synthetic.key" entity package "$fixtures/assembly.p4k" TEST_Ship --out "$packages/assembly"
& $p4kx --json --key-file "$fixtures/synthetic.key" entity package "$fixtures/livery.p4k" TEST_Livery --texture-mip 2 --out "$packages/livery"
& $p4kx --json --key-file "$fixtures/synthetic.key" character package "$fixtures/character.p4k" --chf "$fixtures/hero.chf" --anim all --expression CTL_a --materials colours --out "$packages/character"
```

Use `debug/p4kx` on Linux. Check each command's exit status before continuing. The fixture directory
must be new. The generated key is exclusively `p4kx_testkit::test_key`, not a game Archive key.
The source fixtures intentionally include unresolved attachments and unsupported palette input;
the CLI reports these separately from importer warnings.

Run the consumer with Blender 5.2 or 5.3 in a separate background process:

```powershell
$env:BLENDER_USER_CONFIG = Join-Path $verificationRoot 'blender-config'
$env:BLENDER_USER_SCRIPTS = Join-Path $verificationRoot 'blender-scripts'
$env:BLENDER_USER_DATAFILES = Join-Path $verificationRoot 'blender-data'
$env:PYTHONDONTWRITEBYTECODE = '1'
& 'C:\Program Files\Blender Foundation\Blender 5.3\blender.exe' --background --factory-startup --disable-autoexec --threads 2 --python-exit-code 1 --python blender_addon/tests/verify_exported_package.py -- $packages
```

Give the process a 180-second watchdog and capture stdout/stderr separately. Do not reuse an
interactive Blender process or its configuration. The consumer resets its own scene between
cases and checks these independent authored fixture expectations:

| Case | Scene nodes / drawing instances / mesh assets | Additional checks |
| --- | --- | --- |
| Assembly | 6 / 4 / 3 | Z-up attachment positions, nested parent, quarter-turn, material assignments |
| Livery | 7 / 7 / 4 | Shared collection instances, one shared 2x2 mip texture, connected primary/secondary palette constants |

The Assembly and Livery cases require twelve triangles per cube, finite vertices, authored evaluated world bounds
and no additional importer warnings. Scene-node lookup uses only the imported root's direct
objects, because Blender can give mesh-template objects the same authored name first. Bounds
include the fixture cube's ten-metre mesh-node offset where present.

The Character case imports one armature with three authored bones, one action and one expression
shape key. At 30 fps, the action spans frames 0 through 30. The test samples the evaluated pelvis
at its three authored CAF keys in source Z-up metres with a tolerance of `1e-5` m.

The consumer writes `consumer-report.json` and two fixed-camera 320x240 CPU renders beside the
package folders. The Character row records the Blender build, package and importer hashes, bone
hierarchy, action range and measured positions. The native artifact exchange also verifies the
exporter executable identity and all package-file hashes. The report counts file-backed images
and excludes rendering-only datablocks. Palette checks inspect connected
shader inputs; rendered pixel colour is not a source-fidelity assertion.

The fixtures cover synthetic geometry, materials, one texture path and one absolute rig-bound
animation. Installed archive coverage, additive composition and in-game appearance have separate
acceptance criteria.

## Palette gloss and packed normal alpha

The focused fixture exports a textured GLB and a Blender package from the same synthetic
Archive. It crosses palette endpoints with absent, zero and half `GlossMult`, readable
DDNA alpha, explicit Gloss, alpha-free normals and textureless fallback. A separate AO
wrapper has two contributing layer declarations, vertex colours and explicit zero wear.

```powershell
$env:P4KX_SYNTHETIC_PALETTE_GLOSS_PACKAGE = Join-Path $verificationRoot 'palette-gloss'
cargo test --locked -p p4kx-blender --test it -- palette_gloss_fixture_compares_glb_and_blender_package_inputs --nocapture
& 'C:\Program Files\Blender Foundation\Blender 5.3\blender.exe' --background --factory-startup --disable-autoexec --python-exit-code 23 --python-expr 'import sys; sys.dont_write_bytecode = True' --threads 1 --python blender_addon/tests/blender_material_gloss_package.py -- "$env:P4KX_SYNTHETIC_PALETTE_GLOSS_PACKAGE/scene.json"
```

Use a new package directory and the isolated Blender profile above. The Rust check reads
the embedded packed green channel and exported factor. The ordinary Blender importer is
measured through a one-thread linear EXR emission probe with a fixed orthographic camera;
reflectance and display grading do not affect that scalar signal.

For the selected HardSurface finish, both consumers apply
`R = clamp(1 - S * M * P, 0, 1)`: sampled smoothness `S`, authored layer multiplier `M`
and selected palette gloss `P`. The fixture supplies expected inputs and tolerances; its
printed measurements describe the actual candidate. Without a sample, the selected palette
supplies base smoothness. An absent palette entry uses the documented unpainted fallback.
See [ADR-0400](adr/0400-selected-hardsurface-palettes-multiply-sampled-smoothness.md).

Readable packed normal alpha takes precedence over an explicit Gloss slot. A selected
HardSurface finish reads explicit Gloss red; the generic unselected RGB-Gloss path retains
its luminance convention. Non-grey RGB with independently varying alpha distinguishes all
three inputs. The AO wrapper checks that zero wear selects only the primary layer even
with vertex colour present. Original layer positions, palette selection and image origins
remain part of package verification.

The new alpha-minification control uses constant RGB and varying alpha. Temporarily selecting
RGB minification in that alpha consumer must fail the exact regression; restoring RGBA
minification must pass. Existing RGB, base-colour and normal sampling keep their own behavior.

On native Windows AMD64, Blender 5.3.0 (`02f0899d78e3`) measured neutral roughness
`0.44538248`, half-multiplier roughness `0.72269118`, packed-alpha precedence
`0.23137246` and zero-wear AO wrapper roughness `0.95472515`. The explicit Gloss control
with independently different RGB and alpha measured `0.64065933` in the unselected
luminance path. All eight candidate cases passed their declared quantization tolerances.
These are synthetic scalar-consumer measurements, not a game-appearance comparison.

The ignored `installed_palette_subset::installed_original_subset_preserves_material_and_uv_inputs`
library test in `p4kx-blender` exercises one privately selected original subset through both normal
producers. It requires `P4KX_DATA_P4K`, the Archive key, a private
`P4KX_INSTALLED_PALETTE_SUBSET_SELECTION` file and a fresh empty absolute
`P4KX_INSTALLED_PALETTE_SUBSET_OUTPUT` directory under the agent's scratch root. Review the selected
physical closure and output paths before running that exact ignored test. Include `.dds.a` alpha
headers alongside the selected texture families. No Catalog, full ship or Assembly walk is needed.

`blender_addon/tests/blender_installed_palette_subset.py` consumes the completed private output
with the normal package and glTF importers. Both exported meshes retain all original UV float bits;
the probe compares linear Roughness at four original triangle UV centroids with tolerance `2/255`.
A square orthographic render verifies the projection and distinct foreground identity of every
sample. Four known linear scalar values and two transparent background pixels check the readback.
The selected draw ranges invalidate original first-use vertex traversal, so this test deliberately
requests no vertex-map sidecar. Its direct scene carries no invented Assembly palette provenance.

The first installed-subset readback sampled the background because automatic camera fitting
placed the planes on different rows. The corrected native Blender 5.3.0 (`02f0899d78e3`) readback
on the retained LIVE 13165946 outputs agreed at all four UVs with foreground and scalar controls.
The separate synthetic Gloss regression passed with a freshly generated package. These
consumer-projection checks leave runtime shader activation and game appearance open.

The Release add-on from PR #1117 head `405e97268` matched all 42 signed
package files. Blender 5.3.0 (`02f0899d78e3`) imported the retained LIVE
`13165946` subset in an isolated profile and reproduced the four original
foreground UV comparisons. The maximum absolute Roughness difference
was `0.001093` at tolerance `2/255`. The same exact add-on passed eight
linear scalar controls with a fresh synthetic package from the current
Rust producer. The game-side shader permutation and parameter binding
require a separate runtime witness.
[Release add-on](https://github.com/p4kx-org/p4kx/actions/runs/45315878218/artifacts/14884972989)

A supplementary neutral-light comparison retained the visible material outputs from
both ordinary imports of that same 15,479-triangle Liberator subset at LOD 0 and
mip 2. Blender 5.3.0 used a fixed orthographic camera, identical neutral lighting and
exposure, and Cycles CPU with one thread and eight samples at 192 × 192. Both scene-linear
RGBA EXRs shared all 773 solid foreground pixels. The mean linear RGB was
`(0.9122, 0.7172, 0.5839)` for the package and `(0.6697, 0.5413, 0.4493)` for the GLB.
The mean absolute RGB-channel difference was `0.18456`. This comparison measures the
combined appearance of the exported materials in Blender. The game draw and active
detail inputs require separate runtime evidence.

### Independently bound installed shader operations

The installed LIVE and PTU token vocabularies were independently bound to each engine's
original keyword initializer and direct token-vector consumer. Eighteen complete selected
finish/BRDF helpers compare canonically equal. A separate complete texture-gloss helper
also agrees and reads the scalar `.x` component. These checks verify the selected inner
members and input identities; they do not establish whole-cache equality or a verified
checksum of the large outer cache. The opaque header word has no assigned version meaning.

The selected palette branch multiplies sampled smoothness by layer and palette controls.
Detail preprocessing and its generation flag are separate operations. The direct isotropic
GGX consumer supports P4KX's inverse perceptual roughness coordinate; the source-specific
upper-gloss clamp is outside this projection.

Active runtime permutations, DDS-to-GPU view swizzles, DDNA-alpha binding, CPU palette
packing, exact runtime UVs and rendered game appearance remain separate proof boundaries.
Texture presence alone does not activate detail. Synthetic consumer parity verifies the
authored P4KX projection and cannot attest those runtime facts.

### Packed palette selectors

The LayerBlend_V2 decoder trace reads the packed tint index before multiplying the
smoothness by sampled gloss X. The palette library extracts the one-based slot with
`packedTint & 0x3`, the diffuse flag with `packedTint & 0x4` and the specular flag
with `packedTint & 0x8`. The library multiplies Reflectance by the palette specular
factor when that flag is set and by the original layer specular factor otherwise.
The mapping from authored MTL `TintMode` to the packed value remains open. The
stencil palette function has its own mode.
[Painted surface investigation](https://github.com/p4kx-org/p4kx/issues/1162)
· [Independent tint flags](https://github.com/p4kx-org/p4kx/issues/1163)

On LIVE `13220821`, the two measured Corbel owners each selected a different
named palette through their original worn-material selector. One DataForge
decode verified the Archive directory checksum, the two owner selections and
both target Records. The check measured 1,091,555,328 bytes of peak private
commit. The rigged package still needs a draw-level comparison that binds
each original geometry slot and UV sample to its emitted palette inputs.

## LayerBlend palette tint modes

The LayerBlend fixture exports both a GLB and Blender package from one synthetic Archive.
It checks authored values `0`, `1`, `2` and `3`, an absent value, malformed and nonfinite
tokens, unknown bits and repeating declarations. An unselected layer supplies a control.
The two palette slots and the authored diffuse and specular inputs are distinguishable.
The test checks exact source order and per-value material and palette origins.

`layer_tint_mode_baseline_preserves_raw_values_and_matches_glb_colour` writes the package when
`P4KX_SYNTHETIC_LAYER_TINT_MODE_PACKAGE` names a new absolute directory in the isolated
verification profile. Run the native probe with the settings above and
`blender_addon/tests/blender_layer_tint_modes_package.py -- <package>/scene.json`.

On Windows AMD64 with Blender 5.3.0 (`02f0899d78e3`), both outputs used the same linear
colour for every palette-bound case. The dielectric input was
`(0.52711523, 0.21586053, 0.05126947)` and the metallic input was
`(0.63075721, 0.87136722, 0.52711523)`. Blender's linear EXR probe of Specular IOR Level
returned `0.41352269` for both authored modes `1` and `2`, within one 8-bit step.
The unselected control used the original material colours. These observations measure
the P4KX projection. The engine's CPU flag packing and matched installed comparisons
are separate acceptance evidence.

A second fixture gives two layers distinct child `Diffuse`, wrapper `TintColor` and palette
slots, then blends them with a shared fractional mask. All four GLB texels measure linear
Base Color `(0.06124607, 0.16513222, 0.08437622)`. On the same Blender build, the imported
package measures `(0.11842261, 0.24393596, 0.37463924)`. The GLB bake uses child `Diffuse`
as its authored colour factor. Blender uses wrapper `TintColor`. The Blender probe also
measures Specular IOR Level `0.69820887` from the first resolved layer and Metallic `0`.

`masked_layer_tint_preserves_distinct_authored_inputs_and_measures_glb` writes the package
when `P4KX_SYNTHETIC_LAYER_TINT_MASK_PACKAGE` names a fresh directory in the isolated profile.
Use `blender_addon/tests/blender_layer_tint_mask_package.py -- <package>/scene.json` for the
one-thread native EXR probe.

### Installed factory-painted bow comparison

For LIVE 13220821 and PTU 13248297, the default Assembly resolution selects the factory palette
from root geometry while the paint port is empty. The verified original-subset witness above
covers one starboard-bow submesh with 6,744 triangles and four original UV samples in both Builds.
Blender 5.3.0 measures a maximum linear roughness difference of 0.001796 within the 2/255 tolerance.
The largest linear Base Color difference between the ordinary package and GLB paths is 0.046595.

At the most affected UV and colour channel, disabling the package detail overlay in the probe
changes the difference to 0.056655. Neutralizing the first layer's authored tint as well reduces
it to 0.001214 in both Builds. These results reproduce the authored factor difference in the
masked-layer fixture and the package's detail overlay. The matched-build game comparison is
required for acceptance of the factory-panel appearance. The original material receipts and
diagnostic imports remain private.

## Forward Illum and explicit Emittance

This fixture exports a textured GLB and a Blender package for absent, zero and positive
`Glow`, plus a separate explicit Emittance map. Its original material, diffuse and Emittance
inputs carry independently checked provenance. The GLB test compares the diffuse alpha and
static emission. Blender checks the opacity graph, effective emission, and separate texture
and tint nodes, then renders the explicit Emittance case into a small EXR.

```powershell
$env:P4KX_SYNTHETIC_FORWARD_ILLUM_PACKAGE = Join-Path $verificationRoot 'forward-illum'
cargo test --locked -p p4kx-blender --test it -- forward_illum_package_preserves_original_controls_and_glb_alpha_independently
& 'C:\Program Files\Blender Foundation\Blender 5.3\blender.exe' --background --factory-startup --disable-autoexec --python-exit-code 23 --python-expr 'import sys; sys.dont_write_bytecode = True' --python blender_addon/tests/blender_forward_illum_package.py -- "$env:P4KX_SYNTHETIC_FORWARD_ILLUM_PACKAGE/scene.json"
```

The Forward Illum recipe and the earlier graph-only palette gloss recipe passed on
native Windows AMD64 with Blender 5.3.0
(`02f0899d78e3`). The former rendered explicit Emittance. The latter verified packed alpha and
the palette roughness graph.

## Worn helper placement in Blender

Generate the opt-in worn fixture under a new absolute directory with
`P4KX_TEST_WORN_HELPER_FIXTURE=1` and
`cargo run --locked -p p4kx-testkit --example blender_consumer_fixture -- <fixture-directory>`.
Its `worn-helper.p4k` and synthetic key are test inputs. Export two packages
from the same `Mount_Courier` loadout into separate `original` and
`replacement` subdirectories under an absolute package directory.
The Blender step verifies package format 1.21, the committed file digests and
the worn mount provenance before importing either rig.

```powershell
$arguments = @('--key-file', "$fixtureDirectory/synthetic.key", 'character', 'package',
    "$fixtureDirectory/worn-helper.p4k", '--body', 'male', '--loadout', 'Mount_Courier',
    '--materials', 'colours')
& $p4kx @arguments --name original --out "$packageDirectory/original"
& $p4kx @arguments --piece Mount_New_Carrier --name replacement --vertex-maps --out "$packageDirectory/replacement"
$addon = (Resolve-Path -LiteralPath 'blender_addon').ProviderPath
& $blender --background --factory-startup --offline-mode --disable-autoexec --threads 1 --python-exit-code 1 --python-expr 'import sys; sys.dont_write_bytecode = True' --python blender_addon/tests/blender_worn_helper_package.py -- $addon $packageDirectory
```

Blender 5.3 checks the expected positions of both rigid pieces and their locators,
their spine parent and one drawn vertex following a pose while the pelvis stays fixed.
The original fixture uses a 0.2 m port offset. The replacement uses its own 0.8 m offset.

With direct item-locator alignment, the normal Blender 5.3.0 (`02f0899d78e3`)
import places the original and replacement locators at `(0.00, 0.10, 1.70)` m and
`(0.00, 0.10, 2.30)` m. Both remain aligned with their port through the evaluated
spine pose. On LIVE `13220821` the same GLB export aligns the original Deadlock
and DOOM locators with their authored `Spine3`-relative holders. The main shells
measure about 0.62 m and 0.57 m tall, respectively, in the rest pose.

A Windows AMD64 check of the preceding mount convention on Blender 5.3.0
(`02f0899d78e3`) used a synthetic tagged tree and a fixture-only selector evaluator. The normal exporter retained the root
on `spine` and reported the unresolved selector. With the fixture's explicit matches,
the same tree selected helper ordinals `[1, 0]`, preserved the source positions of the
selected helper fields, and attached the satchel to `head` using its authored item
locator. The fallback locator rested at `(0.00, -0.20, 1.60)` m and the nested
locator at `(0.00, -0.60, 2.30)` m in Blender. Both followed a pose on their
respective bones. The root and explicit torso replacement packages also passed.

A second Windows AMD64 control on the same Blender build used one synthetic
Archive with a padded selector. The ordinary exporter recorded the unresolved
selector and used the root translation. The fixture-only evaluator selected
the nested translation after trimming that token. Both GLBs preserved the
spine parent, item locator and rotation. Under the same spine pose, the mesh,
locator and checked vertex retained the expected difference from the authored
translation. All four package controls passed with verified file digests.

On LIVE build `13220821`, the original Record for an equipped item has the same
source anchor, item locator and rotation on the root and nested helpers. Its
original single-precision Y and Z positions differ, and the projected
placement preserves that difference. Trimming the spaces around the selector
in a diagnostic query isolated the nominated nested helper. The game's
selector matching and fallback behavior remain unverified for this Record.

Two series of LIVE Deadlock captures show the equipped backpack upright. The
corrected export places the two authored attachment alternatives 5.83 cm apart.
The side and elevated views exposed the backpack and parts of the collar. They
did not yield independent rigid `Spine3` landmarks for separating those two
placements. The camera positions agreed between the Hangar and Levski coordinate
frames to within 1 cm per axis. The camera angle and the posed torso could not
be independently calibrated from the repeated floor details and visible collar.

The [static relay fixture](static-relay-blender-verification.md) checks full helper rotation
in an ordinary entity package imported by native Blender.

## Native package exchange in CI

The focused job builds the native P4KX CLI on each row of the
[platform contract](supported-platforms.md). It generates the Assembly, Livery and Character
packages twice with `blender_consumer_fixture` and the normal package export commands, compares
the complete path and byte inventory, then validates the manifests against the Blender package
schema. The uploaded artifact contains the three packages and a receipt for the source revision, workflow run,
native target, exporter digest and every file digest.

Every add-on row downloads all five producer artifacts from that workflow run. Its native Python
process runs the pure add-on tests, checks the exact artifact receipts, validates asset references
with the add-on and compares the decoded scene manifests across targets. This exercises Windows
packages on both Linux CPUs and Linux packages on both Windows CPUs. The native Rust filesystem
tests in the same CI matrix cover the export transaction, including occupied destinations,
missing parents, directory replacement and cleanup.

Windows AMD64, Ubuntu 24.04 AMD64 and macOS ARM64 use the upstream Python 3.13 wheels for
Blender 5.2.3 and 5.3.0. Windows ARM64 uses the 5.2.3 wheel and the official 5.3.0 portable
distribution. Its 5.3.0 job verifies the pinned ZIP digest and the executable's ARM64 PE
machine before running Blender with an isolated profile. Each version runs the same headless
synthetic smoke and imports the exchanged packages to check authored geometry, transforms,
material bindings and rig animation. The optional CPU renders are reserved for local visual
review. [Blender 5.2.3 wheel files](https://pypi.org/project/bpy/5.2.3/#files) ·
[Blender 5.3.0 wheel files](https://pypi.org/project/bpy/5.3.0/#files) ·
[Blender 5.3.0 official ZIP checksums](https://download.blender.org/release/Blender5.3/blender-5.3.0.sha256)

The Ubuntu 24.04 ARM64 row runs the native P4KX producer and pure Python validation directly.
Its native Blender 5.2 headless check runs separately in a pinned ARM64 Debian container using
Blender `5.2.2+dfsg-1`. Debian supplies the required newer glibc and runtime libraries. The
container verifies the host, daemon, image and Python architecture and mounts the exchanged
packages read-only. It uses a dated, signed APT snapshot for build reproducibility. Docker and
Blender are CI consumer dependencies. The image explicitly installs NumPy for glTF import
and cattrs for the extension manager. [Debian ARM64 Blender](https://packages.debian.org/sid/arm64/graphics/blender) · [Debian Snapshot](https://snapshot.debian.org/archive/debian/20261104T000000Z/) · [Docker Official Debian image](https://hub.docker.com/_/debian)

The supported upstream wheels and standalone release files do not include Linux ARM64, and the
Ubuntu 24.04 ARM64 Blender package is 4.0.2. The Flathub package currently targets x86_64.
The Debian check supplies ARM64 Blender 5.2 evidence in that userland. A native Blender 5.3
consumer on Linux ARM64 and a supported native Blender runtime directly on Ubuntu 24.04 ARM64
remain outside this CI coverage. [Ubuntu ARM64 package](https://packages.ubuntu.com/noble/arm64/blender) · [Blender 5.3 release files](https://download.blender.org/release/Blender5.3/) · Flathub architecture manifest

## Stencil palette colour and coverage

This fixture exports a textured GLB and a Blender package for each of two assemblies
from one synthetic Archive. One assembly has three authored decal palette colours.
The other has no palette and uses the material's own stencil colour and red mask.

```powershell
$env:P4KX_SYNTHETIC_STENCIL_PACKAGE = Join-Path $verificationRoot 'stencil'
$env:P4KX_SOURCE_REVISION = (git rev-parse HEAD).Trim()
cargo test --locked -p p4kx-blender --test it -- stencil_palette_colours_are_projected_into_glb_and_the_blender_package
& 'C:\Program Files\Blender Foundation\Blender 5.3\blender.exe' --background --factory-startup --disable-autoexec --threads 1 --python-exit-code 23 --python blender_addon/tests/blender_stencil_package.py -- $env:P4KX_SYNTHETIC_STENCIL_PACKAGE
```

Use the isolated Blender profile above and a new output directory. The test measures
three separate masks, two overlaps, and zero and fractional coverage. It compares
GLB colour in sRGB bytes and coverage in raw alpha bytes. Pure colours and coverage
match exactly, and the mixed red and blue colour permits one byte of quantization.
It measures the imported Blender colour and coverage sockets in two fixed-camera
linear EXRs within 0.012. The one-thread Cycles probe uses fixed sampling and disables
denoising. The script routes the imported sockets through emission on two copies of
the material to measure each signal independently of lights and blend settings.
It records the supplied source revision, Blender build, importer and package-file
hashes, sample values and warnings in `stencil-report.json`. This checks the
override-2 and missing-palette projections of ADR-0310 in both outputs. The ADR
also records the remaining questions about in-game mask priority and stencil fields.

The Rust test `fitted_paint_supplies_the_drawn_stencil_and_its_physical_colour_origins`
reuses the masks with a fitted Paint Record that selects a tagged hull variant. The
GLB and package distinguish the selected palette from the base palette. The package
links all nine colour components to their fields in the encrypted physical palette Record.

Producer revision `256a1960cbdcc74793096031c0864c8bff91d6f1` passed both Rust cases.
Blender 5.3.0 (build `02f0899d78e3`) imported the generated painted and fallback
packages with the release add-on from `405e97268b10c2dd0125e0440d1d6a60d300c8b0`.
The two imports reported zero warnings. Both sets of eight colour and coverage
samples agreed within the 0.012 linear tolerance.
[Release add-on artifact](https://github.com/p4kx-org/p4kx/actions/runs/45315878218/artifacts/14884972989)
