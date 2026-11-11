<#
.SYNOPSIS
Writes SHA256SUMS.txt and SHA3-512SUMS.txt for the assets of a release and assembles its notes.

.DESCRIPTION
Requires the complete native package, SBOM and runtime evidence matrix. Hashes every file in -AssetDirectory (the release assets: ZIPs, MSI, SBOM and attestation
bundles) and writes the two hash files next to them in the usual "<hash>  <file>" format. Then
joins the notes in this order: the CHANGELOG section, the optional VirusTotal section, the
"Build provenance" section and the "Checksums" section. GitHub appends its generated "What's
Changed" list when the release is created with --generate-notes.

Needs PowerShell 7 on a platform whose crypto library supports SHA3-512.

.PARAMETER AssetDirectory
The directory holding exactly the files to publish.

.PARAMETER NotesPath
The CHANGELOG section produced by `cargo xtask release-notes`.

.PARAMETER OutputNotesPath
Where the assembled notes are written.

.PARAMETER VirusTotalNotePath
The section written by virustotal-scan.ps1. Optional; omitted from the notes when absent.

.PARAMETER Repository
The repository in the form owner/name, used in the verification commands.

.PARAMETER Version
The release version, defaulting to the workspace version.

.PARAMETER Revision
The full source commit digest, defaulting to GITHUB_SHA or the current commit.

.PARAMETER TagAttestations
Selects attestation instructions after the tagged workflow has verified completion of both attestation jobs.

.EXAMPLE
./assemble-release.ps1 -AssetDirectory assets -NotesPath release-notes.md -OutputNotesPath notes.md -Repository p4kx-org/p4kx
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$AssetDirectory,
    [Parameter(Mandatory)][string]$NotesPath,
    [Parameter(Mandatory)][string]$OutputNotesPath,
    [string]$VirusTotalNotePath,
    [Parameter(Mandatory)][string]$Repository,
    [string]$Version,
    [string]$Revision,
    [switch]$TagAttestations
)

$ErrorActionPreference = 'Stop'
if (-not $Version) {
    $workspace = & cargo metadata --no-deps --format-version 1 --locked | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Release version query failed.' }
    $Version = ($workspace.packages | Where-Object name -CEQ 'p4kx').version
}
if (-not $Revision) {
    $Revision = if ($env:GITHUB_SHA) { $env:GITHUB_SHA } else { (& git rev-parse HEAD).Trim() }
}
& (Join-Path $PSScriptRoot 'check-release-matrix.ps1') -AssetDirectory $AssetDirectory -Version $Version -Revision $Revision

$sha256Name = 'SHA256SUMS.txt'
$sha3Name = 'SHA3-512SUMS.txt'

if (-not [System.Security.Cryptography.SHA3_512]::IsSupported) {
    throw 'This platform does not support SHA3-512.'
}

function Get-HashLine([System.IO.FileInfo]$File, [scriptblock]$Hasher) {
    $stream = [System.IO.File]::OpenRead($File.FullName)
    try { $hash = & $Hasher $stream } finally { $stream.Dispose() }
    '{0}  {1}' -f [System.Convert]::ToHexStringLower($hash), $File.Name
}

$files = @(Get-ChildItem -LiteralPath $AssetDirectory -File | Sort-Object Name)
if (-not $files) { throw "No assets in '$AssetDirectory'." }

$sha256Lines = $files | ForEach-Object { Get-HashLine $_ { param($s) [System.Security.Cryptography.SHA256]::HashData($s) } }
$sha3Lines = $files | ForEach-Object { Get-HashLine $_ { param($s) [System.Security.Cryptography.SHA3_512]::HashData($s) } }
Set-Content -LiteralPath (Join-Path $AssetDirectory $sha256Name) -Value $sha256Lines -Encoding utf8NoBOM
Set-Content -LiteralPath (Join-Path $AssetDirectory $sha3Name) -Value $sha3Lines -Encoding utf8NoBOM

$provenance = if ($TagAttestations) {
@"
## Build provenance

Every asset except the hash files and the attestation bundles is attested (SLSA build
provenance, Sigstore) and ties the file to the commit and workflow that built it. The SBOM
attestations of the target-specific SBOMs (``*.cdx.json``) cover the packages of that target. Verify a
download with the [GitHub CLI](https://cli.github.com/):

``````
gh attestation verify <file> --repo $Repository
``````

The attestations are also attached as Sigstore bundles: ``*-provenance.sigstore.json`` covers
all assets that are not hash files or bundles, and ``<sbom>.cdx.json.sigstore.json`` belongs to
the SBOM of that name. A download can be verified offline against a bundle:

``````
gh attestation verify <file> --bundle <bundle> --repo $Repository
gh attestation verify <file> --bundle <sbom>.cdx.json.sigstore.json --repo $Repository --predicate-type https://cyclonedx.org/bom
``````
"@
} else {
@'
## Build provenance

This validation run did not generate Sigstore attestations.
'@
}

$checksums = @"
## Checksums

``$sha256Name`` and ``$sha3Name`` list every other asset of this release, one
``<hash>  <file>`` line per file. They are plain text files and open in any text editor.

Windows (PowerShell):

``````
Get-FileHash <file> -Algorithm SHA256
[Convert]::ToHexString([Security.Cryptography.SHA3_512]::HashData([IO.File]::ReadAllBytes("<full path of file>")))
``````

``Get-FileHash`` has no SHA3-512. The second command needs PowerShell 7 and an operating system
with SHA3 support (Windows 11 24H2 or newer); ``openssl`` is usually not installed on Windows.
It prints upper-case hex, so compare ignoring case.

Linux:

``````
sha256sum --check --ignore-missing $sha256Name
openssl dgst -sha3-512 <file>
``````

The hash printed must equal the line of the file in the hash file.
"@

$sections = [System.Collections.Generic.List[string]]::new()
$sections.Add((Get-Content -LiteralPath $NotesPath -Raw -Encoding utf8).TrimEnd())
if ($VirusTotalNotePath -and (Test-Path -LiteralPath $VirusTotalNotePath -PathType Leaf)) {
    $sections.Add((Get-Content -LiteralPath $VirusTotalNotePath -Raw -Encoding utf8).TrimEnd())
}
$sections.Add($provenance.TrimEnd())
$sections.Add($checksums.TrimEnd())
Set-Content -LiteralPath $OutputNotesPath -Value ($sections -join "`n`n") -Encoding utf8NoBOM
