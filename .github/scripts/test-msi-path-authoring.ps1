$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-path-probe-source.ps1')

function Test-P4kxMsiPathAuthoring {
    param([string]$Wix, $Platform, [string]$Folder, [string]$Upgrade, $Default)
    $defaultRows = @($Default.environment | Where-Object { $_[0] -match '^[=-]+PATH$' })
    if ($defaultRows.Count -ne 1 -or $defaultRows[0][1] -cne '[~];[APPLICATIONFOLDER]') { throw 'The release MSI no longer authors its unchanged append operation.' }
    $path = Join-Path $Folder 'synthetic-prepend.msi'
    & (Join-Path $PSScriptRoot 'build-msi.ps1') -Wix $Wix -Target $Platform.target -Version '0.5.0' -Output $path -BinaryFolder $Folder -UserPathPart first
    $info = Assert-P4kxPrependProbe $path $Platform '0.5.0' $Upgrade
    if ($info.product -ceq $Default.product -or $info.package -ceq $Default.package) { throw 'The diagnostic probe shares a product/package identity with the default MSI.' }
    $bindingPath = "$path.probe.json"
    $binding = Get-Content -LiteralPath $bindingPath -Raw
    try {
        foreach ($mutation in @(
            { param($item) $item.schema_version = 2 },
            { param($item) $item.role = 'release' },
            { param($item) $item.path_part = 'last' },
            { param($item) $item.target = 'foreign-target' },
            { param($item) $item.version = '0.6.0' },
            { param($item) $item.revision = 'foreign-revision' },
            { param($item) $item.msi_sha256 = '0' * 64 }
        )) {
            $changed = $binding | ConvertFrom-Json
            & $mutation $changed
            $changed | ConvertTo-Json | Set-Content -LiteralPath $bindingPath -Encoding utf8NoBOM
            $rejected = $false
            try { Assert-P4kxPrependProbe $path $Platform '0.5.0' $Upgrade | Out-Null } catch { $rejected = $true }
            if (-not $rejected) { throw 'An unbound or foreign diagnostic probe was accepted.' }
        }
    }
    finally { $binding | Set-Content -LiteralPath $bindingPath -Encoding utf8NoBOM }
    $unbound = Join-Path $Folder 'synthetic.msi.probe.json'
    $binding | Set-Content -LiteralPath $unbound -Encoding utf8NoBOM
    try {
        $changed = $binding | ConvertFrom-Json
        $changed.msi_sha256 = (Get-FileHash -LiteralPath (Join-Path $Folder 'synthetic.msi') -Algorithm SHA256).Hash.ToLowerInvariant()
        $changed | ConvertTo-Json | Set-Content -LiteralPath $unbound -Encoding utf8NoBOM
        $rejected = $false
        try { Assert-P4kxPrependProbe (Join-Path $Folder 'synthetic.msi') $Platform '0.5.0' $Upgrade | Out-Null } catch { $rejected = $true }
        if (-not $rejected) { throw 'A default append MSI was accepted as a prepend probe.' }
    }
    finally { Remove-Item -LiteralPath $unbound }
}
