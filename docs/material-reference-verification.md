# Material-reference verification

The synthetic fixtures exercise the Archive adapter through `ModelSource`, including the actual
GLB primitive-to-material binding. The commands below use generated test data. Run Blender in a separate disposable process.
Installed-Archive checks remain opt-in and follow the existing acceptance deferral.

## Synthetic checks

Run from the repository with its pinned Rust toolchain:

```powershell
cargo test --locked -p p4kx-model --test it material_library_lookup
cargo test --locked -p p4kx-extraction --test it material_reference
cargo test --locked -p p4kx --test it model_materials
cargo test --locked -p p4kx-blender --test it material_reference
cargo test --locked -p p4kx-blender --test schema
cargo clippy --locked -p p4kx-model -p p4kx-extraction -p p4kx-testkit -p p4kx -p p4kx-gui -p p4kx-blender --all-targets -- -D warnings
```

The tests cover both positive forms, relative/Data/bare names, case and separators, missing and
corrupt targets, physical duplicate ambiguity and strict disk rejection. They assert loaded
source materials, distinct caps/sides primitive bindings, colour/opacity and fallback assignments.
No real files or keys are fixtures.

## Source-built package consumer

Use a new directory under the agent's task root and the isolation settings in
[media-fixture-selection.md](media-fixture-selection.md#rust-exported-synthetic-packages-in-blender).
Do not reuse an interactive Blender scene or installed importer. This recipe is manual and does
not override deferred installed-Archive acceptance. The fixture Archives are unencrypted and contain only
generated test data.

```powershell
$verificationRoot = 'D:/Coding/Agents/Temp/codex/p4kx-1091-consumer'
$fixtures = Join-Path $verificationRoot 'fixtures'
$packages = Join-Path $verificationRoot 'packages'
$env:CARGO_TARGET_DIR = Join-Path $verificationRoot 'build'
$env:TMP = Join-Path $verificationRoot 'tmp'
$env:TEMP = $env:TMP
$env:P4KX_FOLDERS_ROOT = Join-Path $verificationRoot 'profile'
$env:P4KX_LAUNCHER_DIR = Join-Path $verificationRoot 'launcher'
$env:P4KX_GAME_CHECK = 'off'
$env:BLENDER_USER_CONFIG = Join-Path $verificationRoot 'blender-config'
$env:BLENDER_USER_SCRIPTS = Join-Path $verificationRoot 'blender-scripts'
$env:BLENDER_USER_DATAFILES = Join-Path $verificationRoot 'blender-data'
$env:PYTHONDONTWRITEBYTECODE = '1'
New-Item -ItemType Directory -Force -Path $env:TMP, $env:P4KX_LAUNCHER_DIR, $packages | Out-Null
cargo run --locked -p p4kx-testkit --example material_reference_fixture -- $fixtures
cargo build --locked -p p4kx --bin p4kx
$p4kx = Join-Path $env:CARGO_TARGET_DIR 'debug/p4kx.exe'
foreach ($case in @('virtual-root', 'authoring', 'collision-virtual', 'collision-authoring')) {
    $entity = if ($case.StartsWith('collision-')) { 'TEST_RootMaterial' } else { 'TEST_MaterialReference' }
    & $p4kx --json entity package "$fixtures/$case.p4k" $entity --out "$packages/$case"
    if ($LASTEXITCODE -ne 0) { throw "Package conversion failed for $case" }
}
```

Stop after any nonzero exit; do not run the consumer against a stale package. Use the actual
reviewed source commit as the second script argument; on Linux use `debug/p4kx` and local
absolute paths. Invoke Blender 5.2 LTS or 5.3 in a new background process with a 180-second watchdog:

```powershell
& 'C:/Program Files/Blender Foundation/Blender 5.2/blender.exe' --background --factory-startup --disable-autoexec --threads 2 --python-exit-code 1 --python blender_addon/tests/verify_material_references.py -- $packages '<reviewed-source-commit>'
```

The script imports all four actual production packages using the repository importer and checks
the loaded importer module paths before hashing that source. The original two cases require four
caps and eight sides triangles and one mesh instance. Their mapped material library assigns a
split diffuse texture to the caps. The consumer verifies the package image path, 8-by-8 decoded
mip and its connection to the caps' Base Color. The collision cases share the same Testkit
builder as the Rust regression: two models in one folder each use source/material index zero and
library name paint, but resolve to different physical libraries. Their description IDs, actual
Blender materials, colours and 12-triangle bindings must stay distinct. The child is attached to
the root at a six-metre X offset; parent links, local/world positions, separate templates and
authored bounds are checked. The collision cases load no file-backed images. Every case checks
for an absent rig and source/import warnings and writes its own controlled render. The report
records Blender/build, importer tree hashes, the supplied source commit and each package's exact
file hashes. Preserve CLI JSON, stdout/stderr and source/build identity beside the report.
Independently inspect source metadata and raw hashes; a successful render does not establish
source completeness or in-game equivalence.

## Ignored installed-Archive regression

`materials::a_sample_of_models_finds_its_material_library_and_exports_valid_glb` keeps its
existing sample and threshold but propagates selected model-construction failures. The separate
`material_references::both_archives_resolve_selected_virtual_and_authoring_materials` uses
the existing model reader with material loading skipped for discovery, then fully converts the
first available example of each reference shape. It scans at most 4096 candidates no larger than
64 MiB per Archive; no fixed private names enter source. Candidate-probe failures are counted.
After selection any read, material or GLB failure is fatal; it never substitutes another candidate.

Both Archive variables and the normal stored key mechanism are required for evidence for both
installations. As other paired tests do, an unset second Archive emits unavailable coverage and
returns without providing a two-Archive pass. A configured Archive with no representative inside
the bounded scan fails coverage explicitly. These checks are not run by the synthetic suite and
must not be enabled until the existing user deferral is lifted. Keep output private.
