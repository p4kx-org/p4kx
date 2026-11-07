param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'target-sbom-document.ps1')
if (Test-Path -LiteralPath $Directory) { throw 'SBOM contract fixture directory must be new.' }
New-Item -ItemType Directory -Path $Directory | Out-Null
$count = 0
foreach ($binary in @('p4kx','p4kx-gui','p4kx-mcp','p4kx-msi-path')) {
    $folder = Join-Path $Directory $binary
    New-Item -ItemType Directory -Path $folder | Out-Null
    $path = Join-Path $folder "$($binary)_bin.cdx.json"
    $source = [ordered]@{
        bomFormat='CycloneDX';metadata=@{component=@{name=$binary;version='0.5.0'}}
        components=@(@{type='library';name='synthetic-dependency';version='1.0.0'})
    } | ConvertTo-Json -Depth 8
    $source | Set-Content -LiteralPath $path -Encoding utf8NoBOM
    $role = if ($binary -ceq 'p4kx-msi-path') { 'embedded-msi-helper' } else { 'application' }
    $document = Get-P4kxTargetSbomDocument $folder $binary 'aarch64-pc-windows-msvc' '0.5.0' $role
    foreach ($pair in @(@('p4kx:binary',$binary),@('p4kx:target','aarch64-pc-windows-msvc'),@('p4kx:role',$role))) {
        $properties = @($document.metadata.properties | Where-Object { $_.name -ceq $pair[0] })
        if ($properties.Count -ne 1 -or $properties[0].value -cne $pair[1]) { throw 'Shared SBOM role/target binding differs.' }
    }
    foreach ($mutation in @(
        {param($doc) $doc.metadata.component.name='wrong-binary'},
        {param($doc) $doc.metadata.component.version='0.6.0'},
        {param($doc) $doc.bomFormat='wrong-format'},
        {param($doc) $doc.components=@()}
    )) {
        $changed = $source | ConvertFrom-Json
        & $mutation $changed
        $changed | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $path -Encoding utf8NoBOM
        $rejected = $false
        try { Get-P4kxTargetSbomDocument $folder $binary 'aarch64-pc-windows-msvc' '0.5.0' $role | Out-Null } catch { $rejected=$true }
        if (-not $rejected) { throw 'Shared SBOM source mutation was accepted.' }
        $count++
    }
}
"PASS: four shared SBOM roles and $count mutations; no Cargo or native calls."
