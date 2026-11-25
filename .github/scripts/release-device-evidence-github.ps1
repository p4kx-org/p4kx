$ErrorActionPreference = 'Stop'

function Invoke-P4kxIntakeApi([string]$Resource, [switch]$Paginate) {
    $arguments = @('api', '--method', 'GET', $Resource)
    if ($Paginate) { $arguments += @('--paginate', '--slurp') }
    $result = & gh @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Unable to verify device receipt ownership with the GitHub API.' }
    return ($result | ConvertFrom-Json -Depth 100)
}

function Test-P4kxWorkflowRunId([string]$Id) {
    $numeric = 0L
    return $Id -cmatch '^[1-9][0-9]*$' -and [long]::TryParse($Id, [ref]$numeric)
}

function Assert-P4kxIntakeTagContext([string]$Event) {
    if ($env:GITHUB_REPOSITORY -cne 'p4kx-org/p4kx' -or
        $env:GITHUB_REPOSITORY_ID -cnotmatch '^[1-9][0-9]*$' -or
        $env:GITHUB_EVENT_NAME -cne $Event -or
        $env:GITHUB_REF_TYPE -cne 'tag' -or
        $env:GITHUB_REF_NAME -cnotmatch '^v[0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.-]+)?(?:\+[A-Za-z0-9.-]+)?$' -or
        $env:GITHUB_REF -cne "refs/tags/$env:GITHUB_REF_NAME" -or
        $env:GITHUB_SHA -cnotmatch '^[0-9a-f]{40}$') {
        throw 'Device receipt handling requires an owning repository version tag and the expected event.'
    }
    $checkedOut = & git rev-parse --verify HEAD
    if ($LASTEXITCODE -ne 0 -or $checkedOut -cne $env:GITHUB_SHA) {
        throw 'Device receipt checkout does not match the dispatched commit.'
    }
}

function Assert-P4kxIntakeTag {
    $tag = [Uri]::EscapeDataString($env:GITHUB_REF_NAME)
    $tagRef = Invoke-P4kxIntakeApi "repos/$env:GITHUB_REPOSITORY/git/ref/tags/$tag"
    if ($tagRef.ref -cne $env:GITHUB_REF) { throw 'GitHub returned a different release tag.' }

    $object = $tagRef.object
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    while ($object.type -ceq 'tag') {
        if ($object.sha -cnotmatch '^[0-9a-f]{40}$' -or -not $seen.Add($object.sha)) {
            throw 'The release tag has an invalid annotated tag chain.'
        }
        $annotation = Invoke-P4kxIntakeApi "repos/$env:GITHUB_REPOSITORY/git/tags/$($object.sha)"
        if ($annotation.sha -cne $object.sha) { throw 'GitHub returned a different annotated tag.' }
        $object = $annotation.object
    }
    if ($object.type -cne 'commit' -or $object.sha -cne $env:GITHUB_SHA) {
        throw 'The release tag does not resolve to the dispatched commit.'
    }
}

function Test-P4kxIntakeRun($Run, [string]$RunId, [string]$Workflow, [string]$Event) {
    return [string]$Run.id -ceq $RunId -and
        $Run.path -cin @($Workflow, "$Workflow@$env:GITHUB_REF") -and
        $Run.event -ceq $Event -and $Run.status -ceq 'completed' -and
        $Run.head_branch -ceq $env:GITHUB_REF_NAME -and $Run.head_sha -ceq $env:GITHUB_SHA -and
        $Run.repository.full_name -ceq $env:GITHUB_REPOSITORY -and
        [string]$Run.repository.id -ceq $env:GITHUB_REPOSITORY_ID -and
        $Run.head_repository.full_name -ceq $env:GITHUB_REPOSITORY -and
        [string]$Run.head_repository.id -ceq $env:GITHUB_REPOSITORY_ID
}

function Test-P4kxIntakeArtifact($Artifact, [string]$Name) {
    return $Artifact.name -ceq $Name -and $Artifact.expired -eq $false -and
        (Test-P4kxWorkflowRunId ([string]$Artifact.id)) -and
        $Artifact.digest -cmatch '^sha256:[0-9a-f]{64}$'
}

function Test-P4kxIntakeArtifactOwner($Artifact, [string]$RunId) {
    $owner = $Artifact.workflow_run
    return [string]$owner.id -ceq $RunId -and
        [string]$owner.repository_id -ceq $env:GITHUB_REPOSITORY_ID -and
        [string]$owner.head_repository_id -ceq $env:GITHUB_REPOSITORY_ID -and
        $owner.head_sha -ceq $env:GITHUB_SHA
}

function Get-P4kxCandidateArtifact([string]$RunId, [string]$Name) {
    $listed = Invoke-P4kxIntakeApi "repos/$env:GITHUB_REPOSITORY/actions/runs/$RunId/artifacts?name=$Name"
    $artifacts = @($listed.artifacts)
    if ($listed.total_count -ne 1 -or $artifacts.Count -ne 1 -or
        -not (Test-P4kxIntakeArtifact $artifacts[0] $Name) -or
        ($null -ne $artifacts[0].workflow_run -and -not (Test-P4kxIntakeArtifactOwner $artifacts[0] $RunId))) {
        throw "Missing exact candidate artifact '$Name'."
    }
    return [string]$artifacts[0].id
}

function Find-P4kxDeviceIntakeArtifact([string]$CandidateRunId, [ValidateSet('macos', 'windows-desktop')][string]$Family = 'macos') {
    Assert-P4kxIntakeTagContext 'push'
    Assert-P4kxIntakeTag
    if (-not (Test-P4kxWorkflowRunId $CandidateRunId) -or $CandidateRunId -cne $env:GITHUB_RUN_ID) {
        throw 'Device receipt lookup requires the active candidate run ID.'
    }
    $name = "$Family-device-evidence-candidate-$CandidateRunId"
    $pages = @(Invoke-P4kxIntakeApi "repos/$env:GITHUB_REPOSITORY/actions/artifacts?name=$name" -Paginate)
    $candidates = @(foreach ($page in $pages) {
        foreach ($artifact in $page.artifacts) {
            $runId = [string]$artifact.workflow_run.id
            if ((Test-P4kxIntakeArtifact $artifact $name) -and (Test-P4kxWorkflowRunId $runId) -and
                (Test-P4kxIntakeArtifactOwner $artifact $runId)) {
                $artifact
            }
        }
    })
    foreach ($artifact in ($candidates | Sort-Object { [long]$_.id } -Descending)) {
        $runId = [string]$artifact.workflow_run.id
        $run = Invoke-P4kxIntakeApi "repos/$env:GITHUB_REPOSITORY/actions/runs/$runId"
        if ((Test-P4kxIntakeRun $run $runId ".github/workflows/$Family-device-evidence.yml" 'workflow_dispatch') -and
            $run.conclusion -ceq 'success') {
            return @{ 'run-id' = $runId; 'artifact-id' = [string]$artifact.id }
        }
    }
    throw "No successful $Family device receipt intake exists for this candidate run. Submit the receipt, then re-run failed jobs."
}

function Save-P4kxIntakeTrustedRoot([string]$Path) {
    & gh attestation trusted-root | Set-Content -LiteralPath $Path -Encoding utf8NoBOM
    if ($LASTEXITCODE -ne 0 -or (Get-Item -LiteralPath $Path).Length -le 0) {
        throw 'Could not retrieve the GitHub attestation trust root.'
    }
}

function Test-P4kxIntakeSigner($Certificate, [string]$RunId, [string]$Event, [string]$Workflow) {
    $repo = "https://github.com/$env:GITHUB_REPOSITORY"
    $invocation = [regex]::Escape("$repo/actions/runs/$RunId")
    $signer = "$repo/$Workflow"
    return $Certificate.buildTrigger -ceq $Event -and
        $Certificate.runInvocationURI -cmatch "^$invocation(?:/attempts/[1-9][0-9]*)?$" -and
        [string]$Certificate.sourceRepositoryIdentifier -ceq $env:GITHUB_REPOSITORY_ID -and
        $Certificate.sourceRepositoryURI -ceq $repo -and
        $Certificate.sourceRepositoryRef -ceq $env:GITHUB_REF -and
        $Certificate.sourceRepositoryDigest -ceq $env:GITHUB_SHA -and
        $Certificate.subjectAlternativeName.type -ceq 'URI' -and
        $Certificate.subjectAlternativeName.value -cin @("$signer@$env:GITHUB_REF", "$signer@$env:GITHUB_SHA")
}

function Assert-P4kxIntakeAttestation(
    [string]$Subject, [string]$Bundle, [string]$Workflow, [string]$TrustedRoot, [string]$RunId, [string]$Event,
    [string]$PredicateType, [switch]$PassThru
) {
    $arguments = @(
        'attestation', 'verify', $Subject,
        '--repo', $env:GITHUB_REPOSITORY,
        '--bundle', $Bundle,
        '--custom-trusted-root', $TrustedRoot,
        '--signer-workflow', "$env:GITHUB_REPOSITORY/$Workflow",
        '--signer-digest', $env:GITHUB_SHA,
        '--source-ref', $env:GITHUB_REF,
        '--source-digest', $env:GITHUB_SHA,
        '--deny-self-hosted-runners',
        '--format', 'json'
    )
    if ($PredicateType) { $arguments += @('--predicate-type', $PredicateType) }
    $result = & gh @arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Could not verify signed provenance for '$([IO.Path]::GetFileName($Subject))'."
    }
    $verified = @($result | ConvertFrom-Json -Depth 100)
    for ($position = 0; $position -lt $verified.Count; $position++) {
        $certificate = $verified[$position].verificationResult.signature.certificate
        if (-not (Test-P4kxIntakeSigner $certificate $RunId $Event $Workflow)) { continue }
        if ($PassThru) {
            $json = [Text.Json.Nodes.JsonNode]::Parse(($result -join [Environment]::NewLine))
            $statement = $json[$position]['verificationResult']['statement']
            if ($null -eq $statement) { throw 'The verified attestation has no statement.' }
            return $statement.ToJsonString()
        }
        return
    }
    throw "Signed provenance for '$([IO.Path]::GetFileName($Subject))' has the wrong workflow run identity."
}

function Get-P4kxIntakeFile([string]$Directory, [string]$Name) {
    $file = Get-Item -LiteralPath (Join-Path $Directory $Name)
    if ($file -isnot [IO.FileInfo] -or $file.Name -cne $Name -or $file.Length -le 0 -or
        ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "Device evidence input '$Name' is not an exact non-empty regular file."
    }
    return $file.FullName
}

function Get-P4kxIntakeHash([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Assert-P4kxIntakeBinding($Binding, $Expected) {
    if ($Binding -isnot [pscustomobject] -or @($Binding.PSObject.Properties).Count -ne $Expected.Count) {
        throw 'Native device receipt binding has the wrong field set.'
    }
    foreach ($field in $Expected.Keys) {
        $property = $Binding.PSObject.Properties[$field]
        if ($null -eq $property -or $property.Name -cne $field -or
            $property.Value -isnot ($Expected[$field].GetType()) -or $property.Value -cne $Expected[$field]) {
            throw "Native device receipt binding differs in '$field'."
        }
    }
}

function Read-P4kxIntakeRuntime([string]$Path, [string]$Version, [string]$RunId, $Platform) {
    $runtime = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    if ($runtime.workflow_run -cne "https://github.com/$env:GITHUB_REPOSITORY/actions/runs/$RunId") {
        throw 'Native runtime evidence belongs to another workflow run.'
    }
    $pins = & (Join-Path $PSScriptRoot 'check-build-tools.ps1')
    $compiler = ($pins | Where-Object { $_.StartsWith('rust-version=') }).Substring('rust-version='.Length)
    $nextest = ($pins | Where-Object { $_.StartsWith('nextest-version=') }).Substring('nextest-version='.Length)
    Assert-P4kxArchiveEvidence $runtime $Platform $env:GITHUB_SHA $compiler $nextest
    Assert-P4kxRuntimeEvidence $runtime $Platform $Version $env:GITHUB_SHA
    return $runtime
}

function Assert-P4kxIntakeCandidateRun([string]$RunId) {
    Assert-P4kxIntakeTag
    $run = Invoke-P4kxIntakeApi "repos/$env:GITHUB_REPOSITORY/actions/runs/$RunId"
    if (-not (Test-P4kxIntakeRun $run $RunId '.github/workflows/release.yml' 'push') -or
        $run.conclusion -cnotin @('failure', 'success')) {
        throw 'Device evidence requires a completed owning release workflow run at the exact pushed tag and commit.'
    }
}
