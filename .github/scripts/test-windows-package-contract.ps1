param([Parameter(Mandatory)][string]$Directory, [Parameter(Mandatory)][string]$Wix)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'msi-database.ps1')
. (Join-Path $PSScriptRoot 'windows-runtime.ps1')
. (Join-Path $PSScriptRoot 'test-msi-path-authoring.ps1')
. (Join-Path $PSScriptRoot 'msi-build.ps1')
New-Item -ItemType Directory -Force -Path $Directory | Out-Null
$upgrade = '090C8F25-8543-42F0-B40A-1514653149F9'
$metadata = @()
foreach ($row in (Get-P4kxPlatformContract).targets | Where-Object os -EQ windows) {
    $folder = Join-Path $Directory $row.target
    New-Item -ItemType Directory -Force -Path $folder | Out-Null
    $bytes = [byte[]]::new(256)
    $bytes[0] = 0x4d; $bytes[1] = 0x5a; $bytes[60] = 128
    $bytes[128] = 0x50; $bytes[129] = 0x45
    [BitConverter]::GetBytes([uint16]$row.pe_machine).CopyTo($bytes, 132)
    foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) { [IO.File]::WriteAllBytes((Join-Path $folder "$binary.exe"), $bytes) }
    @{target=$row.target; package='msi'} | ConvertTo-Json | Set-Content (Join-Path $folder 'p4kx-distribution.json') -Encoding utf8NoBOM
    $runtimeFile = Join-Path $folder 'vcruntime140.dll'
    [IO.File]::WriteAllBytes($runtimeFile, $bytes)
    $inventory = @{
        target=$row.target; redistributables=@(@{
            name='vcruntime140.dll'; machine=$row.pe_machine
            sha256=(Get-FileHash -LiteralPath $runtimeFile -Algorithm SHA256).Hash.ToLowerInvariant()
            source="Visual Studio/VC/Redist/MSVC/synthetic/$($row.msi_arch)/Microsoft.VC143.CRT/vcruntime140.dll"
        })
    } | ConvertTo-Json -Depth 8
    $inventoryPath = Join-Path $folder 'windows-runtime.json'
    $inventory | Set-Content $inventoryPath -Encoding utf8NoBOM
    'Synthetic packaging contract test; no executable runtime evidence.' | Set-Content (Join-Path $folder 'MICROSOFT-REDISTRIBUTABLE-NOTICE.txt') -Encoding utf8NoBOM
    'Synthetic changelog.' | Set-Content (Join-Path $folder 'CHANGELOG.md') -Encoding utf8NoBOM
    Assert-P4kxWindowsRuntimeInventory -Directory $folder -Platform $row
    foreach ($mutation in @(
        { param($item) $item.target = 'wrong-target' },
        { param($item) $item.redistributables[0].machine = 0 },
        { param($item) $item.redistributables[0].sha256 = 'f' * 64 },
        { param($item) $item.redistributables[0].name = '../vcruntime140.dll' }
    )) {
        $changed = $inventory | ConvertFrom-Json
        & $mutation $changed
        $changed | ConvertTo-Json -Depth 8 | Set-Content $inventoryPath -Encoding utf8NoBOM
        $rejected = $false
        try { Assert-P4kxWindowsRuntimeInventory -Directory $folder -Platform $row } catch { $rejected = $true }
        if (-not $rejected) { throw 'Wrong target/machine/digest/path runtime inventory was accepted.' }
    }
    $inventory | Set-Content $inventoryPath -Encoding utf8NoBOM
    $helper = Join-Path $Directory "$($row.target)-helper/payload"
    New-Item -ItemType Directory -Path $helper | Out-Null
    [IO.File]::WriteAllBytes((Join-Path $helper 'p4kx-msi-path.exe'), $bytes)
    foreach ($name in @('p4kx-msi-helper.json','p4kx-msi-helper.cdx.json')) {
        '{"role":"synthetic-authoring-fixture-not-runtime-evidence"}' | Set-Content -LiteralPath (Join-Path $helper $name) -Encoding utf8NoBOM
    }
    'Synthetic fixture; no native runtime claim.' | Set-Content -LiteralPath (Join-Path $helper 'p4kx-msi-helper-NOTICE.txt') -Encoding utf8NoBOM
    $msi = Join-Path $folder 'synthetic.msi'
    Invoke-P4kxWixMsiBuild $Wix $row '0.5.0' $msi $folder $helper ([IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..')))
    $sidecars = @(Get-ChildItem -LiteralPath $folder -File | Where-Object {
        $_.Name -like '*.runtime.wxs' -or $_.Extension -ieq '.wixpdb'
    })
    if ($sidecars.Count -ne 0) { throw 'WiX source or debug intermediates leaked beside the compiled MSI.' }
    $info = Get-P4kxMsiMetadata $msi
    Assert-P4kxMsiMetadata $info $row '0.5.0' $upgrade
    Test-P4kxMsiPathAuthoring $info
    if ('vcruntime140.dll' -cnotin $info.files) { throw 'Runtime DLL was not included in the MSI.' }
    $metadata += $info
    $original = $info.template
    $info.template = 'Intel;1033'
    $rejected = $false
    try { Assert-P4kxMsiMetadata $info $row '0.5.0' $upgrade } catch { $rejected = $true }
    if (-not $rejected) { throw 'Wrong MSI template CPU was accepted.' }
    $info.template = $original
    $probe = Join-Path $folder 'rollback-probe.msi'
    New-P4kxRollbackMsi $msi $probe
    $probeInfo = Get-P4kxMsiMetadata $probe
    if ($probeInfo.properties.P4kxPathPackageCode -cne $probeInfo.package) { throw 'Rollback probe private PackageCode was not restamped.' }
    if ($probeInfo.product -eq $info.product -or $probeInfo.package -eq $info.package -or
        @($probeInfo.sequence | Where-Object { $_[0] -ceq 'P4kxRollbackProbe' -and $_[1] -eq '6550' }).Count -ne 1) {
        throw 'Rollback probe identity or execution sequence differs.'
    }
    $commitProbe = Join-Path $folder 'commit-rollback-probe.msi'
    New-P4kxRollbackMsi $msi $commitProbe -FailureAt after-commit
    $commitInfo = Get-P4kxMsiMetadata $commitProbe
    $fault = @($commitInfo.custom_actions | Where-Object { $_[0] -ceq 'P4kxRollbackProbe' })
    if ($commitInfo.properties.P4kxPathPackageCode -cne $commitInfo.package -or
        $fault.Count -ne 1 -or [int]$fault[0][1] -ne 9730 -or $fault[0][2] -cne 'P4kxPathHelper' -or
        $fault[0][3] -cne 'invalid-injected-commit-command') { throw 'Later commit-failure probe transport differs.' }
}
foreach ($component in $metadata[0].components) {
    if ($component[0] -in @($metadata[1].components | ForEach-Object { $_[0] })) { throw 'MSI component identities collide across CPUs.' }
}
'PASS: both MSI schemas and bundled DLLs; CPU/runtime-inventory rejection; disjoint identities; actual PackageCode stamping; embedded invocation transport; deferred and later-commit rollback authoring. No MSI installed.'
