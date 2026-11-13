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

`p4kx-testkit` can write two synthetic source Archives for an end-to-end exporter/importer check.
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
cargo build --locked -p p4kx --bin p4kx
$p4kx = Join-Path $env:CARGO_TARGET_DIR 'debug/p4kx.exe'
& $p4kx --json --key-file "$fixtures/synthetic.key" entity package "$fixtures/assembly.p4k" TEST_Ship --out "$packages/assembly"
& $p4kx --json --key-file "$fixtures/synthetic.key" entity package "$fixtures/livery.p4k" TEST_Livery --texture-mip 2 --out "$packages/livery"
```

Use `debug/p4kx` on Linux. Check each command's exit status before continuing. The fixture directory
must be new. The generated key is exclusively `p4kx_testkit::test_key`, not a game Archive key.
The source fixtures intentionally include unresolved attachments and unsupported palette input;
the CLI reports these separately from importer warnings.

Run the consumer with the installed Blender 5.2 executable in a separate background process:

```powershell
$env:BLENDER_USER_CONFIG = Join-Path $verificationRoot 'blender-config'
$env:BLENDER_USER_SCRIPTS = Join-Path $verificationRoot 'blender-scripts'
$env:BLENDER_USER_DATAFILES = Join-Path $verificationRoot 'blender-data'
$env:PYTHONDONTWRITEBYTECODE = '1'
& 'C:\Program Files\Blender Foundation\Blender 5.2\blender.exe' --background --factory-startup --disable-autoexec --threads 2 --python-exit-code 1 --python blender_addon/tests/verify_exported_package.py -- $packages
```

Give the process a 180-second watchdog and capture stdout/stderr separately. Do not reuse an
interactive Blender process or its configuration. The consumer resets its own scene between
cases and checks these independent authored fixture expectations:

| Case | Scene nodes / drawing instances / mesh assets | Additional checks |
| --- | --- | --- |
| Assembly | 6 / 4 / 3 | Z-up attachment positions, nested parent, quarter-turn, material assignments |
| Livery | 7 / 7 / 4 | Shared collection instances, one shared 2x2 mip texture, connected primary/secondary palette constants |

Both cases require twelve triangles per cube, finite vertices, authored evaluated world bounds
and no additional importer warnings. Scene-node lookup uses only the imported root's direct
objects, because Blender can give mesh-template objects the same authored name first. Bounds
include the fixture cube's ten-metre mesh-node offset where present.

The consumer writes `consumer-report.json` and two fixed-camera 320x240 CPU renders beside the
package folders. Inspect both renders and retain source/importer, exporter executable, Blender
executable and package-file hashes with the process result. The report counts file-backed images
and excludes rendering-only datablocks. Palette checks inspect connected
shader inputs; rendered pixel colour is not a source-fidelity assertion.

This check covers synthetic unrigged geometry, materials and one texture path. It does not prove
installed LIVE/PTU coverage, Character rigs, animation, audio, other CPUs or in-game appearance.

## Native package exchange in CI

The focused job builds the native P4KX CLI on each row of the
[platform contract](supported-platforms.md). It generates the assembly and livery packages twice
with `blender_consumer_fixture` and `p4kx entity package`, compares the complete path and byte
inventory, then validates the actual manifests against the Blender package schema. The uploaded
artifact contains only the two packages and a receipt for the source revision, workflow run,
native target, exporter digest and every file digest.

Every add-on row downloads all five producer artifacts from that workflow run. Its native Python
process runs the pure add-on tests, checks the exact artifact receipts, validates asset references
with the add-on and compares the decoded scene manifests across targets. This exercises Windows
packages on both Linux CPUs and Linux packages on both Windows CPUs. The native Rust filesystem
tests in the same CI matrix cover the export transaction, including occupied destinations,
missing parents, directory replacement and cleanup.

Windows AMD64/ARM64, Ubuntu 24.04 AMD64 and macOS ARM64 use the upstream Python 3.13 wheels for
Blender 5.2.3 and 5.3.0. Both run the existing headless synthetic smoke and import the actual
exchanged packages to check authored geometry, transforms and material bindings. The CI import
omits the optional CPU renders used for interactive visual review. [Blender 5.2.3 wheel files](https://pypi.org/project/bpy/5.2.3/#files) · [Blender 5.3.0 wheel files](https://pypi.org/project/bpy/5.3.0/#files)

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
