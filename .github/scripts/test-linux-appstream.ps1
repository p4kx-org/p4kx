<#
.SYNOPSIS
Verifies native AppStream composition and SVG decoding for the shipped desktop assets.
#>
param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
. (Join-Path $PSScriptRoot 'linux-desktop.ps1')
if (-not $IsLinux) { throw 'AppStream composition requires native Linux.' }
$root = [IO.Path]::GetFullPath($Directory)
if ([IO.Path]::Exists($root)) { throw 'AppStream test directory must be new.' }
$payload = Join-Path $root 'payload'
$identity = (Get-P4kxLinuxPackaging).application_id
Install-P4kxLinuxDesktop -Prefix $payload -Version '0.0.0' -ReleaseDate '2026-01-01'

function Assert-P4kxAppStreamComposition {
    <#
    .SYNOPSIS
    Checks composed catalog identity or the exact native image-read failure.
    #>
    param([string]$Name, [switch]$Rejected)
    $output = Join-Path $root $Name
    $data = Join-Path $output 'data'
    $icons = Join-Path $output 'icons'
    $arguments = @(
        'compose', '--no-color', '--print-report=full', '--prefix=/', '--origin=p4kx-contract',
        "--components=$identity", "--result-root=$output", "--data-dir=$data", "--icons-dir=$icons", $payload
    )
    $lines = @(& appstreamcli @arguments 2>&1 | ForEach-Object { $_.ToString() })
    $status = $LASTEXITCODE
    $lines | Set-Content -LiteralPath (Join-Path $root "$Name.log") -Encoding utf8NoBOM
    if ($Rejected) {
        $report = $lines -join "`n"
        if ($status -ne 1 -or $report -cnotmatch 'file-read-error' -or $report -notmatch [regex]::Escape("$identity.svg")) {
            $lines | ForEach-Object { Write-Host $_ }
            throw "Corrupt SVG expected a detailed file-read rejection, got AppStream exit $status."
        }
        return
    }
    if ($status -ne 0) {
        $lines | ForEach-Object { Write-Host $_ }
        throw "Native AppStream composition failed ($status): $Name"
    }
    $catalog = Join-Path $data 'p4kx-contract.xml.gz'
    [xml]$metadata = Invoke-P4kxLinuxTool gzip @('--decompress', '--stdout', $catalog) | Out-String
    if ($identity -cnotin @($metadata.components.component.id) -or
        -not (Test-Path -LiteralPath (Join-Path $icons "64x64/$identity.png") -PathType Leaf)) {
        throw 'AppStream did not publish the expected component and rendered icon.'
    }
}

$previousExit = $global:LASTEXITCODE
try {
    Assert-P4kxAppStreamComposition -Name shipped-icons
    $iconRoot = Join-Path $payload 'share/icons/hicolor'
    foreach ($bitmap in Get-ChildItem -LiteralPath $iconRoot -Recurse -File -Filter '*.png') {
        Remove-Item -LiteralPath $bitmap.FullName
    }
    Assert-P4kxAppStreamComposition -Name svg-only
    $svg = Join-Path $iconRoot "scalable/apps/$identity.svg"
    '<svg invalid' | Set-Content -LiteralPath $svg -Encoding utf8NoBOM
    Assert-P4kxAppStreamComposition -Name corrupt-svg -Rejected
}
finally { $global:LASTEXITCODE = $previousExit }
'PASS: native AppStream composes shipped icons and SVG-only assets, and rejects a corrupt SVG with its filename and decoder report.'
