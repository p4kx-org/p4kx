<#
.SYNOPSIS
Rejects transport, malformed or rebound API4 outcomes without starting any process.
#>
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'flatpak-prerequisites.ps1')
$runtime = '[p4kx.runtime.documents-api5]'
$unsupported = '[p4kx.documents.unsupported-interface:4]'
foreach ($stderr in @("$runtime $unsupported localised explanation",
    "Error: $runtime $unsupported`nCaused by: $unsupported")) {
    $proof = Assert-P4kxFlatpakPrerequisiteOutcome @{exit_code=1; stderr=$stderr} ([long]4)
    if ($proof.refusal_reason -cne 'unsupported-interface' -or $proof.interface_version -isnot [uint32] -or
        $proof.interface_version -ne 4) { throw 'Typed API4 cause/version was not preserved.' }
}
$bad = @(
    @{exit_code=1; stderr="$runtime session bus unavailable (os error -1001)"},
    @{exit_code=1; stderr="$runtime Documents service unavailable"},
    @{exit_code=1; stderr="$runtime document access denied"},
    @{exit_code=1; stderr="$runtime malformed Documents response"},
    @{exit_code=1; stderr="$runtime Documents query timed out"},
    @{exit_code=1; stderr="$runtime Documents interface 4 does not provide host paths"},
    @{exit_code=1; stderr="$unsupported"},
    @{exit_code=1; stderr="$runtime`n$unsupported"},
    @{exit_code=1; stderr="$runtime [p4kx.documents.unsupported-interface:5]"},
    @{exit_code=1; stderr="$runtime [p4kx.documents.unsupported-interface:3]"},
    @{exit_code=1; stderr="$runtime $unsupported [p4kx.documents.unsupported-interface:5]"},
    @{exit_code=1; stderr="$runtime [p4kx.documents.unsupported-interface:04]"},
    @{exit_code=1; stderr="$runtime [p4kx.documents.unsupported-interface:-4]"},
    @{exit_code=1; stderr="$runtime [p4kx.documents.unsupported-interface:4.0]"},
    @{exit_code=1; stderr="$runtime [p4kx.documents.unsupported-interface:4294967295]"},
    @{exit_code=1; stderr="$runtime [p4kx.documents.unsupported-interface:4294967296]"},
    @{exit_code=1; stderr="$runtime $unsupported [p4kx.documents.unsupported-interface:broken]"},
    @{exit_code=0; stderr="$runtime $unsupported"},
    @{exit_code=2; stderr="$runtime $unsupported"},
    @{exit_code='1'; stderr="$runtime $unsupported"},
    @{exit_code=1.0; stderr="$runtime $unsupported"},
    @{exit_code=1; stderr=$null}
)
foreach ($result in $bad) {
    $rejected = $false
    try { Assert-P4kxFlatpakPrerequisiteOutcome $result | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'A transport/malformed/different-version result was accepted as API4 proof.' }
}
foreach ($version in @('4', 4.0, $true, 0, 5, [uint32]::MaxValue)) {
    $rejected = $false
    try { Assert-P4kxFlatpakPrerequisiteOutcome @{exit_code=1; stderr="$runtime $unsupported"} $version | Out-Null }
    catch { $rejected = $true }
    if (-not $rejected) { throw 'An untyped or different measured API version was accepted.' }
}
"PASS: 2 canonical API4 outcomes; $($bad.Count) invalid results and 6 invalid expected-version bindings rejected; no native calls."
