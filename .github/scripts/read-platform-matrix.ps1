param([ValidateSet('windows', 'linux')][string]$Os)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
$contract = Get-P4kxPlatformContract
$include = @(foreach ($row in $contract.targets) {
    if ($Os -and $row.os -cne $Os) { continue }
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
if ($env:GITHUB_OUTPUT) { "matrix=$matrix" | Add-Content -LiteralPath $env:GITHUB_OUTPUT -Encoding utf8NoBOM }
$matrix
