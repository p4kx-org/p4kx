param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$FixtureDirectory,
    [Parameter(Mandatory)][string]$DataDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-installation.ps1')
. (Join-Path $PSScriptRoot 'flatpak-document-identity-cases.ps1')
$platform = Get-P4kxLinuxPlatform $Target
Assert-P4kxDisposableLinuxRunner $platform
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $output) { throw 'Document cases require a fresh directory.' }
$game = Join-Path $output 'fake-game/LIVE'
$inside = Join-Path $game 'inside-export'
$outside = Join-Path $output 'exports'
New-Item -ItemType Directory -Path (Join-Path $game 'Bin64'), $inside, $outside -Force | Out-Null
$archive = Join-Path $game 'Data.p4k'
Copy-Item -LiteralPath (Join-Path $FixtureDirectory 'native-smoke.p4k') -Destination $archive
$case = @{directory=$output; platform=$platform; application=(Get-P4kxLinuxPackaging).application_id;
    game=$game; inside=$inside; outside=$outside; archive=$archive;
    prefix=(Get-P4kxFlatpakDocumentPrefix $platform $DataDirectory);
    documents=[Collections.Generic.List[string]]::new(); source_hash=(Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash;
    hello_hash=(Get-FileHash -LiteralPath (Join-Path $FixtureDirectory 'hello.txt') -Algorithm SHA256).Hash;
    forged_path=(Join-Path $output 'forged-parent/unrelated.p4k')}
$failures = [Collections.Generic.List[Exception]]::new()
$xattr = $false
$proof = $null
try {
    Invoke-P4kxLinuxTool setfattr @('--name=user.document-portal.host-path', "--value=$($case.forged_path)", $archive) | Out-Null
    $xattr = $true
    $source = Add-P4kxDocumentCaseGrant $case $archive
    $outsideGrant = Add-P4kxDocumentCaseGrant $case $outside -Writable
    $insideGrant = Add-P4kxDocumentCaseGrant $case $inside -Writable
    $proof = Test-P4kxDocumentUnknownSource $case $source $insideGrant $outsideGrant
    $proof['forged_underlying_xattr'] = Test-P4kxDocumentUnderlyingForgery $case $source
    $proof['hardlink'] = Test-P4kxDocumentHardlink $case $source
    $proof['game_alias_refusal'] = Test-P4kxDocumentKnownInstallation $case $insideGrant
    $proof['ordinary_copy'] = Test-P4kxDocumentOrdinaryCopy $case
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -cne $case.source_hash) { throw 'Document cases modified the Archive source.' }
    $proof['source_unchanged'] = 'success'
}
catch { $failures.Add($_.Exception) }
finally {
    foreach ($grant in $case.documents) {
        try { Invoke-P4kxLinuxTool flatpak @('document-unexport', $grant) | Out-Null } catch { $failures.Add($_.Exception) }
    }
    if ($xattr) { try { Invoke-P4kxLinuxTool setfattr @('--remove=user.document-portal.host-path', $archive) | Out-Null } catch { $failures.Add($_.Exception) } }
    try {
        if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -cne $case.source_hash) { throw 'Document operation changed the synthetic Archive source.' }
    }
    catch { $failures.Add($_.Exception) }
}
if ($failures.Count -ne 0) { throw [AggregateException]::new('Document grant cases or cleanup failed.', $failures) }
$proof['target'] = $Target
$proof['revision'] = $env:GITHUB_SHA
$proof | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $output 'documents.json') -Encoding utf8NoBOM
