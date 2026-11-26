param([string]$Repository = (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent))
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-native-progress-liveness.ps1')

function Assert-ProgressContract([bool]$Condition, [string]$Failure) {
    if (-not $Condition) { throw $Failure }
}

$release = Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/release.yml')
$workflow = Get-Content -Raw -LiteralPath (Join-Path $Repository '.github/workflows/windows-msi-native-progress.yml')
$dispatch = [regex]::Match($release,'(?ms)^      windows-native-progress-only:\r?\n.*?(?=^      \S|^\S)').Value
Assert-ProgressContract ($dispatch.Contains('default: false')) 'Diagnostic dispatch must remain opt-in.'
Assert-ProgressContract ($release.Contains('!inputs.windows-observation-control-only && !inputs.windows-native-progress-only') -and
    $release.Contains('exclusive: ${{ !inputs.windows-observation-control-only && !inputs.linux-only && !inputs.windows-only }}')) 'A diagnostic dispatch can reach the release package graph or overlap another scope.'
$archive = $workflow.IndexOf('Download the two sealed Windows build artifacts', [StringComparison]::Ordinal)
$guard = $workflow.IndexOf('Require an exclusive branch diagnostics dispatch', [StringComparison]::Ordinal)
$ready = $workflow.IndexOf('Preserve the original observation readiness and candidate binding', [StringComparison]::Ordinal)
$sample = $workflow.IndexOf('Sample the original native MSI after readiness', [StringComparison]::Ordinal)
$upload = $workflow.IndexOf('Preserve the live MSI journal and liveness before Wait', [StringComparison]::Ordinal)
$wait = $workflow.IndexOf('Wait for the original observation after preserving diagnostics', [StringComparison]::Ordinal)
Assert-ProgressContract ($guard -ge 0 -and $guard -lt $archive -and $archive -lt $ready -and
    $ready -lt $sample -and $sample -lt $upload -and $upload -lt $wait) 'Package admission or pre-Wait diagnostic retention is out of order.'
Assert-ProgressContract ($workflow.Contains("`$env:GITHUB_REF_TYPE -cne 'branch'") -and
    $workflow.Contains("artifact-ids: '15146677135,15144744704'") -and $workflow.Contains('digest-mismatch: error') -and
    $workflow -notmatch '(?m)^\s*(id-token|attestations):|name: release-|Stage CollectFinish|record-windows-release-runtime|release-windows-execute') 'Diagnostic replay gained release authority or lost pinned artifact validation.'

$context = @{ digest='a' * 64 }
$launchDigest = 'b' * 64
$readyReceipt = @{ request_sha256=$context.digest; launch_sha256=$launchDigest }
Assert-P4kxMsiProgressReadiness $context $readyReceipt $launchDigest
foreach ($field in @('request_sha256','launch_sha256')) {
    $foreign = $readyReceipt.Clone()
    $foreign[$field] = 'f' * 64
    $refused = $false
    try { Assert-P4kxMsiProgressReadiness $context $foreign $launchDigest } catch { $refused = $true }
    Assert-ProgressContract $refused "A foreign $field authorized diagnostic observation."
}

$self = Get-Process -Id $PID
try { $ticks = $self.StartTime.ToUniversalTime().Ticks } finally { $self.Dispose() }
Assert-ProgressContract ((Get-P4kxMsiObservedProcess $PID $ticks).state -ceq 'running') 'The bound process was not reported as live.'
Assert-ProgressContract ((Get-P4kxMsiObservedProcess $PID ($ticks - 1)).state -ceq 'pidReused') 'A different process at the same PID was reported as the original.'
'PASS: Native MSI diagnostics bind the original readiness and remain outside release acceptance.'
