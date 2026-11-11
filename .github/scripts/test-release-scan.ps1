param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-scan-evidence.ps1')

$version = '0.5.0'
$revision = 'a' * 40
$runId = '123456'
$scanner = Join-Path $PSScriptRoot 'virustotal-scan.ps1'
$assets = Join-Path $Directory 'assets'
New-Item -ItemType Directory -Path $assets -Force | Out-Null
$contract = Get-P4kxPlatformContract
foreach ($row in $contract.targets) {
    foreach ($package in $row.packages) {
        $name = Get-P4kxAssetName -Target $row.target -Version $version -Package $package
        Set-Content -LiteralPath (Join-Path $assets $name) -Value "synthetic bytes for $name" -Encoding utf8NoBOM
    }
}
Set-Content -LiteralPath (Join-Path $assets 'architecture-neutral-addon.zip') -Value 'outside the native scan matrix'
$files = @(Get-P4kxReleaseScanAssets $assets $version | Select-Object -ExpandProperty FullName)
if ($files.Count -ne 12 -or $files -contains (Join-Path $assets 'architecture-neutral-addon.zip')) {
    throw 'The native scan matrix must contain exactly the twelve platform packages.'
}
$digests = @($files | ForEach-Object {
    (Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()
})
$state = @{ mode = 'complete'; requests = 0; uploads = 0; polls = 0; confirmed = 0 }
$analysisId = 'offline-analysis-id'
function Invoke-RestMethod {
    param([string]$Uri, [string]$Method, [int]$TimeoutSec, [hashtable]$Headers)
    if ($Method -cne 'Get' -or $TimeoutSec -le 0 -or $Headers['x-apikey'] -cne 'offline-fixture') {
        throw 'Unexpected HTTP request settings in the offline release scan fixture.'
    }
    if ($Uri -ceq "https://www.virustotal.com/api/v3/analyses/$analysisId") {
        if ($state.mode -cnotin @('uploaded', 'wrong-analysis') -or $state.uploads -ne 1 -or $state.polls -ne 0) {
            throw 'Unexpected analysis poll in the offline release scan fixture.'
        }
        $state.polls++
        $id = if ($state.mode -ceq 'wrong-analysis') { 'foreign-analysis-id' } else { $analysisId }
        return [pscustomobject]@{ data = [pscustomobject]@{
            type = 'analysis'; id = $id; attributes = [pscustomobject]@{ status = 'completed' }
        } }
    }
    if ($Uri -cnotmatch '^https://www\.virustotal\.com/api/v3/files/([0-9a-f]{64})$' -or
        $Matches[1] -cnotin $digests) {
        throw 'Unexpected HTTP call in the offline release scan fixture.'
    }
    $state.requests++
    $digest = $Matches[1]
    $stats = @{ malicious = 1; suspicious = 0; undetected = 12; harmless = 1 }
    if ($digest -ceq $digests[-1]) {
        if ($state.mode -cin @('uploaded', 'wrong-analysis')) {
            if ($state.uploads -eq 0) {
                $response = [Net.Http.HttpResponseMessage]::new([Net.HttpStatusCode]::NotFound)
                throw [Microsoft.PowerShell.Commands.HttpResponseException]::new('offline missing report', $response)
            }
            if ($state.polls -ne 1) { throw 'The final file report must follow analysis completion.' }
            $state.confirmed++
        }
        switch ($state.mode) {
            'wrong-hash' { $digest = 'f' * 64 }
            'pending' { $stats = @{ malicious = 0; suspicious = 0; undetected = 0; harmless = 0 } }
            'transport' { throw 'offline-fixture transport failure' }
            'invalid' { $stats.malicious = '1' }
        }
    }
    return [pscustomobject]@{ data = [pscustomobject]@{
        type = 'file'; id = $digest; attributes = [pscustomobject]@{
            last_analysis_stats = [pscustomobject]$stats
        }
    } }
}
function curl {
    process {
        $KeyConfig = $_
        if ($state.mode -cnotin @('uploaded', 'wrong-analysis') -or $state.uploads -ne 0 -or
            $KeyConfig -cne 'header = "x-apikey: offline-fixture"' -or
            $args -cnotcontains "file=@$($files[-1])" -or
            $args[-1] -cne 'https://www.virustotal.com/api/v3/files') {
            throw 'Unexpected upload in the offline release scan fixture.'
        }
        $state.uploads++
        $global:LASTEXITCODE = 0
        '{"data":{"type":"analysis","id":"' + $analysisId + '"}}'
        '200'
    }
}

function Invoke-Scan([string]$Case, [string]$Key, [bool]$Required) {
    $note = Join-Path $Directory "$Case.md"
    $receipt = Join-Path $Directory "$Case.json"
    & $scanner -Path $files -NotePath $note -ApiKey $Key -RequireComplete:$Required `
        -EvidencePath $receipt -Version $version -Revision $revision -RunId $runId -PollSeconds 0
}

function Assert-Rejected([scriptblock]$Action, [string]$Message) {
    try { & $Action | Out-Null }
    catch {
        if ($_.Exception.Message -like $Message) { return }
        throw
    }
    throw "The required release policy accepted a rejected fixture: $Message"
}

Invoke-Scan 'complete' 'offline-fixture' $true
$complete = Join-Path $Directory 'complete.json'
Assert-P4kxReleaseScanEvidence $complete $assets $version $revision $runId
if ($state.requests -ne $files.Count) { throw 'Each genuine fixture digest must be queried exactly once.' }
$notes = Get-Content -LiteralPath (Join-Path $Directory 'complete.md') -Raw
if ($notes -notmatch '1 of 14 engines') { throw 'Completed reports retain their actual detection counts.' }

foreach ($uploadMode in @('uploaded', 'wrong-analysis')) {
    $state.mode = $uploadMode
    $state.uploads = 0
    $state.polls = 0
    $state.confirmed = 0
    if ($uploadMode -ceq 'uploaded') {
        Invoke-Scan $uploadMode 'offline-fixture' $true
        Assert-P4kxReleaseScanEvidence (Join-Path $Directory "$uploadMode.json") $assets $version $revision $runId
        if ($state.confirmed -ne 1) { throw 'Uploaded evidence requires the final digest-bound report.' }
    } else {
        Assert-Rejected { Invoke-Scan $uploadMode 'offline-fixture' $true } 'Required VirusTotal reports are missing or pending*'
        $note = Get-Content -LiteralPath (Join-Path $Directory "$uploadMode.md") -Raw
        if ($state.confirmed -ne 0 -or (Test-Path -LiteralPath (Join-Path $Directory "$uploadMode.json")) -or
            $note -notmatch 'VirusTotal returned a different analysis') {
            throw 'A foreign analysis cannot authorize a final file report.'
        }
    }
    if ($state.uploads -ne 1 -or $state.polls -ne 1) { throw 'The offline upload must complete the actual analysis request path.' }
}

foreach ($mode in @('pending', 'transport', 'wrong-hash', 'invalid')) {
    $state.mode = $mode
    Assert-Rejected { Invoke-Scan $mode 'offline-fixture' $true } 'Required VirusTotal reports are missing or pending*'
    $note = Get-Content -LiteralPath (Join-Path $Directory "$mode.md") -Raw
    if ((Test-Path -LiteralPath (Join-Path $Directory "$mode.json")) -or
        $note -notmatch 'analysis pending|not scanned' -or $note.Contains('offline-fixture')) {
        throw "Required scan case '$mode' lost its diagnostic or admitted incomplete evidence."
    }
}
Assert-Rejected { Invoke-Scan 'no-secret' '' $true } 'Required VirusTotal scan is unavailable*'
if (Test-Path -LiteralPath (Join-Path $Directory 'no-secret.json')) {
    throw 'A missing secret cannot produce scan evidence.'
}
Invoke-Scan 'manual' 'offline-fixture' $false
$manual = Get-Content -LiteralPath (Join-Path $Directory 'manual.md') -Raw
if ($manual -notmatch 'not scanned' -or (Test-Path -LiteralPath (Join-Path $Directory 'manual.json'))) {
    throw 'The manual dry-run diagnostic cannot masquerade as a complete tag scan.'
}

$original = Get-Content -LiteralPath $complete -Raw
$mutant = Join-Path $Directory 'mutant.json'
foreach ($mutate in @(
    { param($proof) $proof.rows = @($proof.rows | Select-Object -Skip 1) },
    { param($proof) $proof.rows[-1].name = $proof.rows[0].name },
    { param($proof) $proof.rows[-1].name = 'architecture-neutral-addon.zip' },
    { param($proof) $proof.rows[0].sha256 = 'f' * 64 },
    { param($proof) $proof.rows[0].engines = 0 },
    { param($proof) $proof.rows[0].engines = '14' },
    { param($proof) $proof.revision = 'b' * 40 },
    { param($proof) $proof.run_id = '123457' },
    { param($proof) $proof.version = '0.5.1' }
)) {
    $proof = $original | ConvertFrom-Json
    & $mutate $proof
    $proof | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $mutant -Encoding utf8NoBOM
    Assert-Rejected { Assert-P4kxReleaseScanEvidence $mutant $assets $version $revision $runId } 'VirusTotal evidence*'
}
Assert-Rejected { Assert-P4kxReleaseScanEvidence (Join-Path $Directory 'missing.json') $assets $version $revision $runId } '*missing.json*'
Add-Content -LiteralPath $files[0] -Value 'changed package bytes'
Assert-Rejected { Assert-P4kxReleaseScanEvidence $complete $assets $version $revision $runId } 'VirusTotal evidence does not identify the exact bytes*'

$workflow = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../workflows/release.yml') -Raw
$assemble = [regex]::Match($workflow, '(?ms)^  assemble:\s*\r?\n(.*?)^  publish:')
if (-not $assemble.Success -or
    $assemble.Value -notmatch "needs\.scan\.result == 'success'" -or
    $assemble.Value -notmatch '(?s)Assert-P4kxReleaseScanEvidence.*assemble-release\.ps1' -or
    $workflow -notmatch 'continue-on-error: \$\{\{ github\.ref_type != ''tag''' -or
    $workflow -notmatch '(?m)^    needs: \[prepare, assemble\]\r?$') {
    throw 'The required scan must gate tag assembly before the publishing dependency can succeed.'
}
Write-Output 'Native release scan evidence and dry-run policies passed offline.'
