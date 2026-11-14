<#
.SYNOPSIS
Uploads release assets to VirusTotal and writes a Markdown "Security check" section.

.DESCRIPTION
Hashes every file given with -Path (SHA-256), looks the hash up on VirusTotal, uploads the file
when VirusTotal does not know it yet, waits for the analysis and writes one table row per file
(detections and a report link) to -NotePath. Works on Windows and Linux with PowerShell 7.

Best-effort scans report unavailable or pending results in the note. A required scan
also writes exact package evidence and fails if a completed report is unavailable.

.PARAMETER Path
The files to scan.

.PARAMETER NotePath
Where the Markdown section is written. It is always written, also when the scan did not run.

.PARAMETER ApiKey
The VirusTotal API key. Defaults to the environment variable VIRUSTOTAL_API_KEY.

.PARAMETER TimeoutMinutes
How long to wait for each analysis before the report is linked as pending.

.PARAMETER RequireComplete
Requires a completed digest-bound report for every package.

.PARAMETER EvidencePath
The new JSON receipt for a complete required scan.

.PARAMETER Version
The exact release version for the receipt.

.PARAMETER Revision
The full source commit for the receipt.

.PARAMETER RunId
The workflow run identity for the receipt.

.PARAMETER PollSeconds
The pause between two polls of an analysis.

.EXAMPLE
./virustotal-scan.ps1 -Path P4KX-1.0.0-x86_64.msi, P4KX-1.0.0-x86_64-portable.zip -NotePath virustotal.md
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string[]]$Path,
    [Parameter(Mandatory)][string]$NotePath,
    [string]$ApiKey = $env:VIRUSTOTAL_API_KEY,
    [int]$TimeoutMinutes = 10,
    [int]$PollSeconds = 30,
    [switch]$RequireComplete,
    [string]$EvidencePath,
    [string]$Version,
    [string]$Revision,
    [string]$RunId
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'release-scan-evidence.ps1')
if ($RequireComplete -and (-not $EvidencePath -or -not $Version -or
    $Revision -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$' -or $RunId -cnotmatch '^[1-9][0-9]*$' -or
    (Test-Path -LiteralPath $EvidencePath))) {
    throw 'A required VirusTotal scan needs new evidence and exact release identity.'
}

$api = 'https://www.virustotal.com/api/v3'
$directUploadLimit = 32MB
$uploadTimeoutSeconds = 300
$maxReasonLength = 120
$heading = '## Security check'

function Write-Note([string]$Body) {
    Set-Content -LiteralPath $NotePath -Value $Body -Encoding utf8NoBOM
}

# Keeps a message safe for public release notes: plain characters only, no key, capped length.
function Get-SafeText([string]$Text) {
    if ($ApiKey) { $Text = $Text.Replace($ApiKey, '') }
    $plain = ($Text -replace '[^A-Za-z0-9 .,:;()/_-]', ' ' -replace '\s+', ' ').Trim()
    if ($plain.Length -gt $maxReasonLength) { $plain = $plain.Substring(0, $maxReasonLength) + '...' }
    $plain
}

function Write-Unavailable([string]$Reason) {
    $safe = Get-SafeText $Reason
    Write-Host "::warning title=VirusTotal::$safe - the release notes say the scan did not run."
    Write-Note "$heading`n`nThe VirusTotal scan did not run for this release ($safe)."
    if ($RequireComplete) { throw "Required VirusTotal scan is unavailable: $safe" }
}

function Invoke-Vt {
    param([string]$Uri, [int]$TimeoutSec = 120)
    Invoke-RestMethod -Uri $Uri -Method Get -TimeoutSec $TimeoutSec -Headers @{
        'x-apikey' = $ApiKey
        'accept'   = 'application/json'
    }
}

function Get-HttpStatus($ErrorRecord) {
    if ($ErrorRecord.Exception.Response) { return [int]$ErrorRecord.Exception.Response.StatusCode }
    return 0
}

function Send-MultipartFile([string]$Uri, [System.IO.FileInfo]$File) {
    $keyConfig = 'header = "x-apikey: {0}"' -f $ApiKey
    $lines = $keyConfig | & curl --config - `
        --silent --show-error --fail-with-body `
        --max-time $uploadTimeoutSeconds `
        --header 'accept: application/json' `
        --form "file=@$($File.FullName)" `
        --write-out '\n%{http_code}' `
        --url $Uri
    $exit = $LASTEXITCODE
    $all = @($lines)
    $status = 0
    if ($all.Count) { [int]::TryParse(("$($all[-1])").Trim(), [ref]$status) | Out-Null }
    $body = if ($all.Count -gt 1) { ($all[0..($all.Count - 2)]) -join "`n" } else { '' }
    [pscustomobject]@{
        Ok       = ($exit -eq 0 -and $status -ge 200 -and $status -lt 300)
        Status   = $status
        Body     = $body
        CurlExit = $exit
    }
}

function Get-FileReport([string]$Sha256) {
    try { $report = Invoke-Vt "$api/files/$Sha256" }
    catch {
        if ((Get-HttpStatus $_) -eq 404) { return $null }
        throw
    }
    if ($report.data.type -cne 'file' -or $report.data.id -cne $Sha256) {
        throw 'The VirusTotal report does not identify the requested SHA-256.'
    }
    return $report
}

function Get-FileStats([System.IO.FileInfo]$File, [string]$Sha256) {
    $report = Get-FileReport $Sha256
    if ($report) { return $report.data.attributes.last_analysis_stats }

    $uploadUrl = if ($File.Length -le $directUploadLimit) { "$api/files" } else { (Invoke-Vt "$api/files/upload_url").data }
    Write-Host "Uploading $($File.Name)..."
    $response = Send-MultipartFile $uploadUrl $File
    if (-not $response.Ok) {
        $why = if ($response.Status) { "HTTP $($response.Status)" } else { "curl exit $($response.CurlExit)" }
        throw "upload rejected ($why)"
    }
    $submitted = ($response.Body | ConvertFrom-Json).data
    if ($submitted.type -cne 'analysis' -or $submitted.id -isnot [string] -or -not $submitted.id) {
        throw 'VirusTotal did not return an analysis identifier.'
    }
    $analysisId = [uri]::EscapeDataString($submitted.id)

    $deadline = (Get-Date).AddMinutes($TimeoutMinutes)
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Seconds $PollSeconds
        try { $analysis = Invoke-Vt "$api/analyses/$analysisId" }
        catch {
            Write-Host "  poll failed ($(Get-SafeText $_.Exception.Message)) - retrying."
            continue
        }
        if ($analysis.data.type -cne 'analysis' -or $analysis.data.id -cne $submitted.id) {
            throw 'VirusTotal returned a different analysis.'
        }
        Write-Host "  $($File.Name): $($analysis.data.attributes.status)"
        if ($analysis.data.attributes.status -ceq 'completed') {
            $confirmed = Get-FileReport $Sha256
            if ($confirmed.data.attributes.last_analysis_stats) {
                return $confirmed.data.attributes.last_analysis_stats
            }
        }
    }
    return $null
}

function Get-AnalysisCounts($Stats) {
    if (-not $Stats) { return $null }
    foreach ($name in @('malicious', 'suspicious', 'undetected', 'harmless')) {
        $value = $Stats.PSObject.Properties[$name].Value
        if (-not (Test-P4kxScanInteger $value) -or $value -gt [int]::MaxValue) {
            throw 'VirusTotal returned invalid engine counts.'
        }
    }
    $flagged = [long]$Stats.malicious + [long]$Stats.suspicious
    $engines = $flagged + [long]$Stats.undetected + [long]$Stats.harmless
    if ($engines -gt 0) { return @{ flagged = $flagged; engines = $engines } }
    return $null
}

if (-not $ApiKey) {
    Write-Unavailable 'VIRUSTOTAL_API_KEY is not set'
    return
}
$receipts = [Collections.Generic.List[object]]::new()
try {
    $rows = foreach ($item in $Path) {
        $file = Get-Item -LiteralPath $item -ErrorAction Stop
        if ($file.PSIsContainer -or $file.Length -le 0) { throw "Invalid package at '$item'." }
        $sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        $link = "[report](https://www.virustotal.com/gui/file/$sha256)"
        try { $counts = Get-AnalysisCounts (Get-FileStats $file $sha256) }
        catch {
            $status = Get-HttpStatus $_
            $hint = if ($status -eq 401) { 'the API key was rejected' }
                    elseif ($status -eq 429) { 'the API quota is exhausted' }
                    else { Get-SafeText $_.Exception.Message }
            Write-Host "::warning title=VirusTotal::$($file.Name): $hint"
            "| ``$($file.Name)`` | not scanned ($hint) | $link |"
            continue
        }
        if (-not $counts) {
            Write-Host "::warning title=VirusTotal::$($file.Name): no analysis result yet."
            "| ``$($file.Name)`` | analysis pending | $link |"
            continue
        }
        $receipts.Add([ordered]@{
            name = $file.Name; sha256 = $sha256; flagged = $counts.flagged; engines = $counts.engines
        })
        Write-Host "$($file.Name): $($counts.flagged) / $($counts.engines) engines flagged the file."
        "| ``$($file.Name)`` | $($counts.flagged) of $($counts.engines) engines | $link |"
    }
    Write-Note @"
$heading

Reports are identified by the SHA-256 of the packaged file.

| File | Detections | Report |
|:-----|:-----------|:-------|
$($rows -join "`n")

An unsigned installer or archive that is new on every release can draw individual false
positives; the report shows which engine says what.
"@
}
catch {
    Write-Unavailable "unexpected error: $($_.Exception.Message)"
    return
}

if ($RequireComplete) {
    if ($receipts.Count -ne $Path.Count) {
        throw 'Required VirusTotal reports are missing or pending. See the scan notes.'
    }
    $evidence = [ordered]@{
        schema = 1; version = $Version; revision = $Revision; run_id = $RunId; rows = $receipts.ToArray()
    }
    $evidence | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $EvidencePath -Encoding utf8NoBOM
}
