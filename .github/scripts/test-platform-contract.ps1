$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
$contract = Get-P4kxPlatformContract
$source = $contract | ConvertTo-Json -Depth 10

foreach ($mutation in @(
    { param($c) $c.targets = @($c.targets | Select-Object -Skip 1) },
    { param($c) $c.targets[1] = $c.targets[0] },
    { param($c) $c.targets[0].target = 'aarch64-unknown-linux-gnu' },
    { param($c) $c.targets[0].packages = @('msi', 'deb') },
    { param($c) $c.packages.msi = '../outside.msi' },
    { param($c) $c.version = 2 }
)) {
    $changed = $source | ConvertFrom-Json
    & $mutation $changed
    $rejected = $false
    try { ConvertFrom-P4kxPlatformContract -Json ($changed | ConvertTo-Json -Depth 10) | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Invalid platform contract was accepted.' }
}

$names = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
foreach ($row in $contract.targets) {
    foreach ($package in $row.packages) {
        $name = Get-P4kxAssetName -Target $row.target -Version '0.5.0' -Package $package
        if (-not $name.Contains($row.target) -or -not $names.Add($name)) { throw 'Asset target collision.' }
    }
}
if ($names.Count -ne 12) { throw 'Required native package matrix is incomplete.' }
$current = Get-P4kxCurrentPlatform
$other = $contract.targets | Where-Object target -CNE $current.target | Select-Object -First 1
$rejected = $false
try { Assert-P4kxNativeHost -Platform $other } catch { $rejected = $true }
if (-not $rejected) { throw 'A foreign native host target was accepted.' }
Write-Output 'PASS: platform mutations, twelve unique package identities and foreign-host rejection.'
