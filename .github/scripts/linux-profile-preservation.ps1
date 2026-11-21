$ErrorActionPreference = 'Stop'

function New-P4kxProfileSentinels {
    param([string]$Directory)
    $paths = @('config/P4KX/settings.toml', 'config/P4KX/archive-key-sentinel', 'data/P4KX/keep.dat')
    return @(foreach ($relative in $paths) {
        $path = Join-Path $Directory $relative
        if (Test-Path -LiteralPath $path) { throw 'Profile sentinel path already exists.' }
        New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($path)) | Out-Null
        [IO.File]::WriteAllText($path, "Synthetic package-preservation sentinel: $relative`n", [Text.UTF8Encoding]::new($false))
        @{path=$path; sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash}
    })
}

function Assert-P4kxProfileSentinels {
    param([object[]]$Sentinels)
    foreach ($sentinel in $Sentinels) {
        if (-not (Test-Path -LiteralPath $sentinel.path -PathType Leaf) -or
            (Get-FileHash -LiteralPath $sentinel.path -Algorithm SHA256).Hash -cne $sentinel.sha256) {
            throw 'Package installation, execution or removal changed existing profile data.'
        }
    }
}
