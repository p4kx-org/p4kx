$ErrorActionPreference = 'Stop'

<#
.SYNOPSIS
Reads one generated binary SBOM and binds it to its actual target and distribution role.
#>
function Get-P4kxTargetSbomDocument {
    param([string]$ManifestDirectory, [string]$Binary, [string]$Target, [string]$Version, [string]$Role = 'application')
    $files = @(Get-ChildItem -LiteralPath $ManifestDirectory -Recurse -File -Filter "$($Binary)_bin.cdx.json")
    if ($files.Count -ne 1) { throw "Expected one generated SBOM for $Binary, got $($files.Count)." }
    $sbom = Get-Content -LiteralPath $files[0].FullName -Raw | ConvertFrom-Json
    if ($sbom.bomFormat -cne 'CycloneDX' -or $sbom.metadata.component.version -cne $Version -or
        $sbom.metadata.component.name -cne $Binary -or -not $sbom.components) { throw 'Generated SBOM application identity/components differ.' }
    $properties = @($sbom.metadata.properties | Where-Object { $_ -and $_.name -cnotin @('p4kx:target','p4kx:binary','p4kx:role') })
    $properties += @(@{name='p4kx:target'; value=$Target}, @{name='p4kx:binary'; value=$Binary}, @{name='p4kx:role';value=$Role})
    $sbom.metadata | Add-Member -NotePropertyName properties -NotePropertyValue $properties -Force
    return $sbom
}

function Add-P4kxRuntimeSbomComponents {
    param($Sbom, $Runtime, $Platform)
    if ($null -ne $Runtime -and $Runtime.target -cne $Platform.target) { throw 'Runtime SBOM inventory target mismatch.' }
    foreach ($dll in $Runtime.redistributables) {
        if ($dll.sha256 -cnotmatch '^[0-9a-f]{64}$' -or $dll.machine -ne $Platform.pe_machine) { throw 'Runtime SBOM file has wrong CPU/digest.' }
        $Sbom.components += [pscustomobject]@{
            type='file'; name=$dll.name; 'bom-ref'="microsoft-runtime:$($Platform.target)/$($dll.name)"
            hashes=@(@{alg='SHA-256'; content=$dll.sha256})
            licenses=@(@{license=@{name='Microsoft Visual Studio redistribution terms';url='https://visualstudio.microsoft.com/license-terms/'}})
            properties=@(@{name='p4kx:runtime-source';value=$dll.source})
        }
    }
}
