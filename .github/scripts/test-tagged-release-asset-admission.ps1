param(
    [Parameter(Mandatory)][hashtable]$Assembly,
    [Parameter(Mandatory)][hashtable]$Desktop,
    [Parameter(Mandatory)][string]$FixtureDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-asset-inventory.ps1')

function Assert-Rejected([scriptblock]$Action, [string]$Message) {
    $failure = try { & $Action | Out-Null } catch { $_.Exception.Message }
    if (-not $failure -or -not $failure.Contains($Message, [StringComparison]::Ordinal)) {
        throw "Expected release rejection '$Message'; observed '$failure'."
    }
}

function Save-TestBundle([string]$Path, $Report) {
    ConvertTo-Json -InputObject @($Report) -Depth 40 | Set-Content -LiteralPath $Path -Encoding utf8NoBOM
}

function New-TestBundle($Entry, [string]$Directory) {
    $subjects = @(foreach ($name in $Entry.subjects) {
        @{ name = $name; digest = @{ sha256 = (Get-FileHash -LiteralPath (Join-Path $Directory $name)).Hash.ToLowerInvariant() } }
    })
    $predicate = if ($Entry.sbom) { Get-Content -LiteralPath (Join-Path $Directory $Entry.sbom) -Raw | ConvertFrom-Json }
    else { @{ buildType = 'synthetic fixture' } }
    $repo = "https://github.com/$env:GITHUB_REPOSITORY"
    return @{
        verificationResult = @{
            signature = @{ certificate = @{
                buildTrigger = 'push'
                runInvocationURI = "$repo/actions/runs/$env:GITHUB_RUN_ID/attempts/1"
                sourceRepositoryIdentifier = $env:GITHUB_REPOSITORY_ID
                sourceRepositoryURI = $repo
                sourceRepositoryRef = $env:GITHUB_REF
                sourceRepositoryDigest = $env:GITHUB_SHA
                subjectAlternativeName = @{ type = 'URI'; value = "$repo/$($Entry.workflow)@$env:GITHUB_REF" }
            } }
            statement = @{ subject = $subjects; predicateType = $Entry.predicate; predicate = $predicate }
        }
    }
}

function Assert-FileRejection([string]$Path, [string]$Message) {
    $away = Join-Path $FixtureDirectory 'held-asset'
    Move-Item -LiteralPath $Path -Destination $away
    try { Assert-Rejected { & $assembler @Assembly @Desktop } $Message }
    finally { Move-Item -LiteralPath $away -Destination $Path }
}

function Assert-BundleRejection([string]$Name, [scriptblock]$Mutation, [string]$Message) {
    $path = Join-Path $Assembly.AssetDirectory $Name
    $original = [IO.File]::ReadAllBytes($path)
    $report = @(Get-Content -LiteralPath $path -Raw | ConvertFrom-Json)[0]
    & $Mutation $report.verificationResult
    Save-TestBundle $path $report
    try { Assert-Rejected { & $assembler @Assembly @Desktop } $Message }
    finally { [IO.File]::WriteAllBytes($path, $original) }
}

$variables = @{
    GITHUB_REPOSITORY = $Assembly.Repository
    GITHUB_REPOSITORY_ID = '123'
    GITHUB_SHA = $Assembly.Revision
    GITHUB_RUN_ID = $Desktop.ReleaseRunId
    GITHUB_REF = "refs/tags/v$($Assembly.Version)"
    GITHUB_EVENT_NAME = 'push'
    RUNNER_TEMP = $FixtureDirectory
}
$before = @{}
foreach ($name in $variables.Keys) {
    $before[$name] = [Environment]::GetEnvironmentVariable($name)
    [Environment]::SetEnvironmentVariable($name, $variables[$name])
}
$assembler = Join-Path $PSScriptRoot 'assemble-release.ps1'
$assets = $Assembly.AssetDirectory
try {
    function gh {
        $global:LASTEXITCODE = 0
        if ($args[0] -ceq 'attestation' -and $args[1] -ceq 'trusted-root') {
            '{}'
            return
        }
        if ($args[0] -cne 'attestation' -or $args[1] -cne 'verify') {
            throw 'The synthetic test must only observe the real attestation verification request.'
        }
        $bundle = $args[[Array]::IndexOf($args, '--bundle') + 1]
        $path = $args[[Array]::IndexOf($args, '--custom-trusted-root') + 1]
        $predicate = $args[[Array]::IndexOf($args, '--predicate-type') + 1]
        $signer = $args[[Array]::IndexOf($args, '--signer-workflow') + 1]
        $entry = @($plan | Where-Object name -CEQ ([IO.Path]::GetFileName($bundle)))
        if ($entry.Count -ne 1 -or $predicate -cne $entry[0].predicate -or
            $signer -cne "$env:GITHUB_REPOSITORY/$($entry[0].workflow)" -or
            [IO.Path]::GetFileName($args[2]) -cne $entry[0].subjects[0] -or
            -not (Test-Path -LiteralPath $path -PathType Leaf) -or '--deny-self-hosted-runners' -cnotin $args -or
            $args[[Array]::IndexOf($args, '--source-digest') + 1] -cne $env:GITHUB_SHA -or
            $args[[Array]::IndexOf($args, '--source-ref') + 1] -cne $env:GITHUB_REF) {
            throw 'The release did not request the required attestation verification policy.'
        }
        Get-Content -LiteralPath $bundle -Raw
    }

    Assert-Rejected { & $assembler @Assembly @Desktop } 'Missing tagged release assets:'
    $plan = @(Get-P4kxTaggedReleaseBundlePlan $Assembly.Version)
    if ($plan.Count -ne 21) { throw 'The five-target release requires 21 bundles.' }
    foreach ($row in (Get-P4kxPlatformContract).targets) {
        $prefix = "P4KX-$($Assembly.Version)-$($row.target)"
        if ($row.os -cin @('linux', 'windows')) {
            Set-Content -LiteralPath (Join-Path $assets "$prefix-package-tools.json") -Value '{"scope":"synthetic"}'
        }
        if ($row.os -ceq 'linux') {
            foreach ($format in $row.packages) {
                $package = Get-P4kxAssetName -Target $row.target -Version $Assembly.Version -Package $format
                Set-Content -LiteralPath (Join-Path $assets "$package.producer.json") -Value '{"scope":"synthetic"}'
            }
        }
    }
    $addon = Join-Path $assets "p4kx_importer-$($Assembly.Version).zip"
    Set-Content -LiteralPath $addon -Value 'synthetic Blender extension'
    foreach ($entry in $plan) {
        Save-TestBundle (Join-Path $assets $entry.name) (New-TestBundle $entry $assets)
    }
    & $assembler @Assembly @Desktop

    $checksums = @('SHA256SUMS.txt', 'SHA3-512SUMS.txt')
    $initial = @{}
    foreach ($checksum in $checksums) {
        $path = Join-Path $assets $checksum
        $lines = @(Get-Content -LiteralPath $path)
        if ($lines.Count -ne 69 -or ($lines -cmatch '  SHA(?:256|3-512)SUMS[.]txt$')) {
            throw 'All 69 admitted assets must appear once in each checksum file.'
        }
        $initial[$checksum] = Get-Content -LiteralPath $path -Raw
    }
    & $assembler @Assembly @Desktop | Out-Null
    foreach ($checksum in $checksums) {
        if ((Get-Content -LiteralPath (Join-Path $assets $checksum) -Raw) -cne $initial[$checksum]) {
            throw 'A retry included the previous checksum outputs.'
        }
    }

    Assert-FileRejection $addon 'Missing tagged release assets:'
    Assert-FileRejection (Join-Path $assets $plan[0].name) 'Missing tagged release assets:'
    $extra = Join-Path $assets 'foreign-target-runtime.json'
    Set-Content -LiteralPath $extra -Value 'synthetic unexpected asset'
    try { Assert-Rejected { & $assembler @Assembly @Desktop } "Unexpected tagged release asset 'foreign-target-runtime.json'." }
    finally { Remove-Item -LiteralPath $extra }

    Assert-BundleRejection $plan[0].name {
        param($result) $result.statement.subject[1].digest.sha256 = 'a' * 64
    } 'unexpected subject name or SHA-256'
    Assert-BundleRejection $plan[0].name {
        param($result) $result.statement.subject[1].name = $result.statement.subject[0].name
    } 'unexpected subject name or SHA-256'
    Assert-BundleRejection $plan[1].name {
        param($result) $result.statement.predicate.metadata.component.name = 'other'
    } 'describes a different CycloneDX SBOM'
    Assert-BundleRejection $plan[-1].name {
        param($result) $result.signature.certificate.runInvocationURI = "https://github.com/$env:GITHUB_REPOSITORY/actions/runs/9000"
    } 'wrong workflow run identity'
} finally {
    foreach ($name in $before.Keys) { [Environment]::SetEnvironmentVariable($name, $before[$name]) }
}

'PASS: exact tagged asset and signed statement admission, complete checksums and deterministic retry.'
