param([ValidateSet('windows', 'linux', 'macos')][string]$Os)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
$contract = Get-P4kxPlatformContract
$include = @(foreach ($row in $contract.targets) {
    if ($Os -and $row.os -ne $Os) { continue }
    $count = if ($row.os -eq 'windows' -and $row.arch -eq 'amd64') { 4 } else { 2 }
    [ordered]@{
        target = $row.target
        runner = $row.runner
        os = $row.os
        arch = $row.arch
        partitions = @(1..$count)
        count = $count
    }
})
$matrix = @{ include = $include } | ConvertTo-Json -Depth 8 -Compress
if ($env:GITHUB_OUTPUT) {
    $outputs = @("matrix=$matrix")
    if (-not $Os -or $Os -eq 'macos') {
        $macos = @{ include = @($include | Where-Object os -CEQ 'macos') } | ConvertTo-Json -Depth 8 -Compress
        $outputs += "macos-matrix=$macos"
    }
    $outputs | Add-Content -LiteralPath $env:GITHUB_OUTPUT -Encoding utf8NoBOM
}
$matrix
