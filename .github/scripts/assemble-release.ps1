<#
.SYNOPSIS
Writes SHA256SUMS.txt and SHA3-512SUMS.txt for the assets of a release and assembles its notes.

.DESCRIPTION
Requires the complete native package, SBOM and runtime evidence matrix. Hashes every file in -AssetDirectory (the release assets: ZIPs, MSI, SBOM and attestation
bundles) and writes the two hash files next to them in the usual "<hash>  <file>" format. Then
joins the notes in this order: the CHANGELOG section, the optional VirusTotal section, the
"Build provenance" section and the "Checksums" section. The optional GitHub body links to the
complete changelog when the assembled text exceeds the release body limit.

Needs PowerShell 7 on a platform whose crypto library supports SHA3-512.

.PARAMETER AssetDirectory
The directory holding exactly the files to publish.

.PARAMETER NotesPath
The CHANGELOG section produced by `cargo xtask release-notes`.

.PARAMETER OutputNotesPath
Where the complete assembled notes are written.

.PARAMETER OutputReleaseBodyPath
Where the GitHub release body is written.

.PARAMETER VirusTotalNotePath
The section written by virustotal-scan.ps1. Optional; omitted from the notes when absent.

.PARAMETER Repository
The repository in the form owner/name, used in the verification commands.

.PARAMETER Version
The release version, defaulting to the workspace version.

.PARAMETER Revision
The full source commit digest, defaulting to GITHUB_SHA or the current commit.

.PARAMETER TagAttestations
Requires the exact tagged publication assets and verified attestations before emitting checksums.

.EXAMPLE
./assemble-release.ps1 -AssetDirectory assets -NotesPath release-notes.md -OutputNotesPath notes.md -Repository p4kx-org/p4kx
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$AssetDirectory,
    [Parameter(Mandatory)][string]$NotesPath,
    [Parameter(Mandatory)][string]$OutputNotesPath,
    [string]$OutputReleaseBodyPath,
    [string]$VirusTotalNotePath,
    [Parameter(Mandatory)][string]$Repository,
    [string]$Version,
    [string]$Revision,
    [switch]$TagAttestations,
    [string]$WindowsDesktopEvidencePath,
    [string]$ReleaseRunId
)

$ErrorActionPreference = 'Stop'
if ($OutputReleaseBodyPath) {
    $completePath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputNotesPath)
    $bodyPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputReleaseBodyPath)
    $comparison = if ($IsWindows) { [StringComparison]::OrdinalIgnoreCase } else { [StringComparison]::Ordinal }
    if ($completePath.Equals($bodyPath, $comparison)) { throw 'Complete notes and GitHub body require separate paths.' }
}
if ($TagAttestations -and (-not $WindowsDesktopEvidencePath -or -not $ReleaseRunId)) {
    throw 'Tagged release assembly requires signed Windows AMD64 desktop evidence for its candidate run.'
}
if (-not $Version) {
    $workspace = & cargo metadata --no-deps --format-version 1 --locked | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'Release version query failed.' }
    $Version = ($workspace.packages | Where-Object name -CEQ 'p4kx').version
}
if (-not $Revision) {
    $Revision = if ($env:GITHUB_SHA) { $env:GITHUB_SHA } else { (& git rev-parse HEAD).Trim() }
}
& (Join-Path $PSScriptRoot 'check-release-matrix.ps1') -AssetDirectory $AssetDirectory -Version $Version -Revision $Revision -WindowsDesktopEvidencePath $WindowsDesktopEvidencePath -ReleaseRunId $ReleaseRunId -RequireStableMacosRelease:$TagAttestations

$macosRuntimePath = Join-Path $AssetDirectory "P4KX-$Version-aarch64-apple-darwin-runtime.json"
$macosRuntime = Get-Content -LiteralPath $macosRuntimePath -Raw | ConvertFrom-Json
$signing = @($macosRuntime.package_smokes)[0].signing
if ($TagAttestations -and $signing.reason -ceq 'package-dry-run') {
    throw 'A dry-run DMG cannot be published.'
}
$macosDistribution = if ($signing.mode -ceq 'developer-id') {
    "## macOS distribution" + [char]10 + [char]10 +
        'The Apple Silicon DMG uses Developer ID signing and a validated stapled Apple notarization ticket.'
} else {
    $reason = switch -CaseSensitive ($signing.reason) {
        'package-dry-run' { 'this was a package validation run' }
        'developer-id-unavailable' { 'Developer ID was unavailable' }
        'notarization-unavailable' { 'Apple notarization was unavailable' }
        'developer-id-and-notarization-unavailable' { 'Developer ID and Apple notarization were unavailable' }
    }
    @"
## macOS distribution

The Apple Silicon DMG is ad-hoc signed because $reason. After attempting to open the
installed P4KX app, use **System Settings → Privacy & Security → Open Anyway** if
Gatekeeper requires approval. Follow [Apple’s instructions](https://support.apple.com/en-us/102445).
"@
}

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

$checksumNames = @($sha256Name, $sha3Name)
$files = if ($TagAttestations) {
    . (Join-Path $PSScriptRoot 'release-asset-inventory.ps1')
    $plan = @(Get-P4kxTaggedReleaseBundlePlan $Version)
    Get-P4kxTaggedReleaseFiles $AssetDirectory $plan $checksumNames | Sort-Object Name
} else {
    Get-ChildItem -LiteralPath $AssetDirectory -File | Where-Object Name -CNotIn $checksumNames | Sort-Object Name
}
if (-not $files) { throw "No assets in '$AssetDirectory'." }

$digests = [Collections.Generic.Dictionary[string, string]]::new([StringComparer]::Ordinal)
foreach ($file in $files) {
    $digests.Add($file.Name, (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant())
}
if ($TagAttestations) {
    . (Join-Path $PSScriptRoot 'release-attestation-evidence.ps1')
    Assert-P4kxTaggedReleaseAttestations $AssetDirectory $plan $digests $Repository $Version $Revision $ReleaseRunId
}
$sha256Lines = $files | ForEach-Object { '{0}  {1}' -f $digests[$_.Name], $_.Name }
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

$changes = (Get-Content -LiteralPath $NotesPath -Raw -Encoding utf8).TrimEnd()
$sections = [System.Collections.Generic.List[string]]::new()
if ($VirusTotalNotePath -and (Test-Path -LiteralPath $VirusTotalNotePath -PathType Leaf)) {
    $sections.Add((Get-Content -LiteralPath $VirusTotalNotePath -Raw -Encoding utf8).TrimEnd())
}
$sections.Add($macosDistribution.TrimEnd())
$sections.Add($provenance.TrimEnd())
$sections.Add($checksums.TrimEnd())
$details = $sections -join "`n`n"
$notes = $changes + "`n`n" + $details
if ($OutputReleaseBodyPath) {
    $githubReleaseBodyLimit = 125000
    $limit = $githubReleaseBodyLimit - [Environment]::NewLine.Length
    $body = $notes
    if ($body.Length -gt $limit) {
        $separator = "`n`n---`n`n"
        $normalizedChanges = $changes.Replace("`r`n", "`n")
        $noticeStart = $normalizedChanges.LastIndexOf($separator, [StringComparison]::Ordinal)
        if ($noticeStart -lt 0) { throw 'The changelog notes have no legal notice separator.' }
        $changelog = "https://github.com/$Repository/blob/$Revision/CHANGELOG.md"
        $link = "## Changes`n`n[Complete changelog for P4KX $Version]($changelog)"
        $body = $link + $normalizedChanges.Substring($noticeStart) + "`n`n" + $details
        if ($body.Length -gt $limit) { throw 'Required release information exceeds the GitHub release body limit.' }
    }
    Set-Content -LiteralPath $OutputReleaseBodyPath -Value $body -Encoding utf8NoBOM
}
Set-Content -LiteralPath $OutputNotesPath -Value $notes -Encoding utf8NoBOM
