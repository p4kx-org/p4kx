# Static relay placement in Blender

Use the same checkout for the fixture generator, P4KX CLI and Blender add-on.
The four-entry fixture is synthetic and requires no installed game archive or key.
Its only port authors `noRotation = true`. Two translated helpers apply rotations
about different axes, so translation-only placement leaves the fuse incorrectly oriented.

Use the [isolated verification profile](media-fixture-selection.md#rust-exported-synthetic-packages-in-blender).
Set `$p4kx` and `$blender` to the built P4KX executable and Blender 5.3 executable.
Generate the fixtures in a new absolute directory if they are not already prepared.

```powershell
cargo run --locked -p p4kx-testkit --example blender_consumer_fixture -- $fixtures
```

Keep the archive, source listings and package in the private verification directory.
Check each command's exit status before proceeding.

```powershell
$relayArchive = Join-Path $fixtures 'relay.p4k'
$relayCheck = Join-Path $verificationRoot 'relay'
$relayRecords = Join-Path $relayCheck 'records'
$relayModel = Join-Path $relayCheck 'source-model.json'
$relayPackage = Join-Path $relayCheck 'package'
[void][IO.Directory]::CreateDirectory($relayCheck)
& $p4kx verify $relayArchive --json
& $p4kx dcb export $relayArchive --filter 'entities/test_relay' --format json --out $relayRecords
& $p4kx model info $relayArchive 'Data/objects/test/relay.cgf' --json |
    Set-Content -LiteralPath $relayModel -Encoding utf8
& $p4kx entity package $relayArchive TEST_Relay --lod 0 --out $relayPackage --json
$relayRecord = Join-Path $relayRecords 'entities/test_relay.json'
& $blender --background --factory-startup --offline-mode --disable-autoexec --threads 1 `
    --python-exit-code 1 --python-expr 'import sys; sys.dont_write_bytecode = True' `
    --python blender_addon/tests/blender_static_relay_package.py -- $relayPackage $relayModel $relayRecord $relayArchive
```

The verifier reads the authored port flag and composes the two original model helper
matrices independently of the package. It verifies the import receipt and the evaluated
fuse's hierarchy, drawable mesh, parent-relative affine and world affine. Its
translation-only control runs through the same importer in an empty scene using
an in-memory replacement rotation. The control keeps the same translation and
must differ in orientation. The verifier hashes all input files before and after import
and reports the Blender build.

The native Blender 5.3.0 import emitted no warnings on Windows AMD64.
The CLI reported one missing material library for the synthetic mesh.
The world and relative matrices agreed within `2.23e-16` per coefficient. The
translation-only control had zero position error and a maximum matrix error of `1.0`.
The witness covers static placement. The animation-time interpretation remains
unresolved under [ADR-0321](adr/0321-a-ports-ang3-offset-is-degrees-with-x-before-z-and-a-loadout-entry-prefers-its-reference.md).
