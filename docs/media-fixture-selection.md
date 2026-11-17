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

`p4kx-testkit` can write three synthetic source Archives for an end-to-end exporter/importer check.
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

The focused material fixture exports a textured GLB and a Blender package from the same
synthetic Archive. It crosses palette bytes `0`, `1`, `128`, `255` and `184` with authored
`GlossMult` of `0.5` and `0`. Byte `184` also has a case with the multiplier absent.
Each case has materials with and without a normal map. The checks cover the palette origin,
the authored layer multiplier and packed DDNA alpha `196 / 255`.

```powershell
$env:P4KX_SYNTHETIC_PALETTE_GLOSS_PACKAGE = Join-Path $verificationRoot 'palette-gloss'
cargo test --locked -p p4kx-blender --test it -- palette_gloss_fixture_compares_glb_and_blender_package_inputs --nocapture
& 'C:\Program Files\Blender Foundation\Blender 5.3\blender.exe' --background --factory-startup --disable-autoexec --python-exit-code 23 --python-expr 'import sys; sys.dont_write_bytecode = True' --threads 1 --python blender_addon/tests/blender_material_gloss_package.py -- "$env:P4KX_SYNTHETIC_PALETTE_GLOSS_PACKAGE/scene.json"
```

Use a new package directory and the isolated Blender profile above. The GLB test reads the
packed roughness channel from the embedded image and multiplies it by the exported factor.
Blender checks the packed alpha pixels and the linked Roughness graph. For palette byte `184`,
a one-thread 64x64 linear EXR probe samples the imported Roughness inputs through emission.
The probe keeps a fixed orthographic camera and excludes reflectance and display grading.
The sampled neutral and authored-half GLB cases expect `59 / 255` after the factor.
Blender expects `59 / 255` with the
multiplier absent and `157 / 255` with authored `0.5`, within one 8-bit step. The textureless
fallback expects `71 / 255` and `163 / 255`, respectively. Cargo and Blender print the measured
sampled values. The fixture measures the P4KX projection. Installed shader arithmetic, Details
and in-game appearance have separate criteria.

On Windows AMD64 with Blender 5.3.0 (`02f0899d78e3`), the sampled GLB cases
measured `0.231372550` with absent, zero and `0.5` multipliers. The Blender
linear probe measured `0.231372461` with the multiplier absent and
`0.615686238` with `0.5`. The shared stencil probe also verified all eight
painted and eight fallback samples.

For an unlayered HardSurface material, the GLB uses the explicit `_gloss` map when the
normal map supplies no readable smoothness alpha. It chooses the first usable authored
Gloss slot. Both consumers sample the decoded RGB as data and use the Blender 5.3
default Linear Rec.709 luminance for the explicit map. The GLB quantizes the inverse
to its packed green channel and keeps the existing specular map's dimensions and blue
channel when present. The paired numeric rule applies to the same texel under the
default Blender colour configuration and neutral own-material multiplier. Blender
can sample a separate UV transform per map. The GLB combines channels into one texture.
[Blender default colour configuration](https://github.com/blender/blender/blob/v5.3.0/release/datafiles/colormanagement/config.ocio)

The shared synthetic Archive also supplies an encrypted explicit Gloss texture with RGBA
`[32, 96, 224, 17]`. The package verifies the original PNG's data encoding and encrypted
source. Both the gloss-only case and the alpha-free normal case export GLB green `163`,
or `0.639215686`, and measure Blender roughness `0.640659332`. The alpha-free case keeps
its Normal input linked. Adding a usable DDNA alpha byte `196` selects it ahead of Gloss.
That case exports green `59` and measures `0.231372461` in Blender. The measured pairs
agree within half an 8-bit step. These Windows AMD64 measurements use Blender 5.3.0
(`02f0899d78e3`) and the package from the ordinary Rust producer.

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
remain outside this CI coverage. [Ubuntu ARM64 package](https://packages.ubuntu.com/noble/arm64/blender) · [Blender 5.3 release files](https://download.blender.org/release/Blender5.3/) · [Flathub architecture manifest](https://github.com/flathub/org.blender.Blender/blob/master/flathub.json)

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
GLB colour in sRGB bytes and coverage in raw alpha bytes, each within one byte.
It measures the imported Blender colour and coverage sockets in two fixed-camera
linear EXRs within 0.012. The one-thread Cycles probe uses fixed sampling and disables
denoising. The script routes the imported sockets through emission on two copies of
the material to measure each signal independently of lights and blend settings.
It records the supplied source revision, Blender build, importer and package-file
hashes, sample values and warnings in `stencil-report.json`. This checks the
override-2 and missing-palette projections of ADR-0310 in both outputs. The ADR
also records the remaining questions about in-game mask priority and stencil fields.
