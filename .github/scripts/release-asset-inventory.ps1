$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')

function Get-P4kxTaggedReleaseBundlePlan([string]$Version) {
    $contract = Get-P4kxPlatformContract
    foreach ($row in $contract.targets) {
        $prefix = "P4KX-$Version-$($row.target)"
        $packages = @(foreach ($format in $row.packages) {
            Get-P4kxAssetName -Target $row.target -Version $Version -Package $format
        })
        $sboms = @(foreach ($binary in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
            "$prefix-$binary.cdx.json"
        })
        $metadata = @("$prefix-runtime.json")
        if ($row.os -cin @('linux', 'windows')) {
            $metadata += "$prefix-package-tools.json"
        }
        if ($row.os -ceq 'linux') {
            $metadata += "$prefix-appimage-corresponding-source.tar.gz"
            $metadata += @(foreach ($name in $packages) { "$name.producer.json" })
        }
        $workflow = '.github/workflows/release-attest.yml'
        [pscustomobject]@{
            name = "$prefix-provenance.sigstore.json"
            workflow = $workflow
            predicate = 'https://slsa.dev/provenance/v1'
            subjects = $packages + $sboms + $metadata
            sbom = $null
        }
        foreach ($sbom in $sboms) {
            [pscustomobject]@{
                name = "$sbom.sigstore.json"
                workflow = $workflow
                predicate = 'https://cyclonedx.org/bom'
                subjects = $packages
                sbom = $sbom
            }
        }
    }
    [pscustomobject]@{
        name = "P4KX-$Version-addon-provenance.sigstore.json"
        workflow = '.github/workflows/release.yml'
        predicate = 'https://slsa.dev/provenance/v1'
        subjects = @("p4kx_importer-$Version.zip")
        sbom = $null
    }
}

function Get-P4kxTaggedReleaseFiles([string]$Directory, [object[]]$Plan, [string[]]$ChecksumNames) {
    $required = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($entry in $Plan) {
        foreach ($name in @($entry.name) + $entry.subjects) {
            $null = $required.Add($name)
        }
    }
    foreach ($file in Get-ChildItem -LiteralPath $Directory -Force) {
        if ($file -isnot [IO.FileInfo] -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Release asset '$($file.Name)' is not a regular file."
        }
        if ($file.Name -cin $ChecksumNames) { continue }
        if (-not $required.Remove($file.Name)) {
            throw "Unexpected tagged release asset '$($file.Name)'."
        }
        if ($file.Length -le 0) { throw "Empty tagged release asset '$($file.Name)'." }
        $file
    }
    if ($required.Count) {
        throw "Missing tagged release assets: $(($required | Sort-Object) -join ', ')."
    }
}
