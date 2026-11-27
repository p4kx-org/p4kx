$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-device-evidence-github.ps1')

function Assert-P4kxReleaseStatement($Entry, [string]$StatementJson,
    [Collections.Generic.Dictionary[string, string]]$Digests, [string]$AssetDirectory) {
    $statement = ConvertFrom-Json -InputObject $StatementJson -Depth 100
    if ($statement.predicateType -cne $Entry.predicate -or $statement.subject -isnot [array] -or
        $statement.subject.Count -ne $Entry.subjects.Count) {
        throw "Attestation '$($Entry.name)' has the wrong predicate or number of subjects."
    }
    $remaining = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($name in $Entry.subjects) { $null = $remaining.Add($name) }
    foreach ($subject in $statement.subject) {
        if ($subject.name -isnot [string] -or -not $remaining.Remove($subject.name) -or
            $subject.digest.sha256 -isnot [string] -or $subject.digest.sha256 -cne $Digests[$subject.name]) {
            throw "Attestation '$($Entry.name)' has an unexpected subject name or SHA-256."
        }
    }
    if ($Entry.sbom) {
        $actual = [Text.Json.Nodes.JsonNode]::Parse($StatementJson)['predicate']
        $expected = [Text.Json.Nodes.JsonNode]::Parse([IO.File]::ReadAllText((Join-Path $AssetDirectory $Entry.sbom)))
        if (-not [Text.Json.Nodes.JsonNode]::DeepEquals($expected, $actual)) {
            throw "Attestation '$($Entry.name)' describes a different CycloneDX SBOM."
        }
    }
}

function Assert-P4kxTaggedReleaseAttestations(
    [string]$AssetDirectory, [object[]]$Plan, [Collections.Generic.Dictionary[string, string]]$Digests,
    [string]$Repository, [string]$Version, [string]$Revision, [string]$RunId
) {
    if ($Repository -cne $env:GITHUB_REPOSITORY -or $Revision -cne $env:GITHUB_SHA -or
        $RunId -cne $env:GITHUB_RUN_ID -or $env:GITHUB_REF -cne "refs/tags/v$Version" -or
        $env:GITHUB_EVENT_NAME -cne 'push' -or -not [IO.Path]::IsPathFullyQualified($env:RUNNER_TEMP)) {
        throw 'Tagged attestation validation requires the exact release run identity.'
    }
    $trust = Join-Path $env:RUNNER_TEMP "p4kx-release-trusted-root-$RunId.json"
    Save-P4kxIntakeTrustedRoot $trust
    foreach ($entry in $Plan) {
        $anchor = Join-Path $AssetDirectory $entry.subjects[0]
        $bundle = Join-Path $AssetDirectory $entry.name
        $statement = Assert-P4kxIntakeAttestation $anchor $bundle $entry.workflow $trust $RunId 'push' -PredicateType $entry.predicate -PassThru
        Assert-P4kxReleaseStatement $entry $statement $Digests $AssetDirectory
    }
}
