# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-tools.ps1')
if (Test-Path -LiteralPath $Directory) { throw 'Download contract fixture directory must be new.' }
New-Item -ItemType Directory -Path $Directory | Out-Null
$script:payload = [Text.Encoding]::UTF8.GetBytes('synthetic complete pinned tool')
$script:partial = [Text.Encoding]::UTF8.GetBytes('partial')
$script:url = 'https://invalid.example/synthetic-pinned-tool'
$script:responses = [Collections.Generic.List[Net.Http.HttpResponseMessage]]::new()
$script:caseCount = 0
$digestPath = Join-Path $Directory 'expected.bin'
[IO.File]::WriteAllBytes($digestPath, $script:payload)
$script:expectedDigest = (Microsoft.PowerShell.Utility\Get-FileHash -LiteralPath $digestPath -Algorithm SHA256).Hash.ToLowerInvariant()

function New-TestHttpFailure {
    param([int]$Status)
    $response = [Net.Http.HttpResponseMessage]::new([Net.HttpStatusCode]$Status)
    $script:responses.Add($response)
    return [Microsoft.PowerShell.Commands.HttpResponseException]::new('synthetic HTTP failure', $response)
}

function New-TestNetworkFailure {
    param([Net.Sockets.SocketError]$Reason)
    return [Net.Http.HttpRequestException]::new('synthetic transport failure', [Net.Sockets.SocketException]::new([int]$Reason))
}

function Invoke-WebRequest {
    param($Uri, $OutFile, $MaximumRetryCount = 0, $ErrorAction = 'Stop')
    if ($Uri -cne $script:url -or $OutFile -cne $script:state.path -or $MaximumRetryCount -ne 0 -or $ErrorAction -cne 'Stop') {
        throw 'Download changed its pinned URL, destination or cmdlet retry policy.'
    }
    $position = $script:state.calls
    $script:state.calls++
    if ($position -ge $script:state.steps.Count) { throw 'Unexpected additional download attempt.' }
    $step = $script:state.steps[$position]
    [IO.File]::WriteAllBytes($OutFile, $step.bytes)
    if ($null -ne $step.failure) { throw $step.failure }
}

function Start-Sleep {
    param($Seconds)
    if ($Seconds -ne 2) { throw 'Download retry interval differs.' }
    $script:state.sleeps++
}

function Get-FileHash {
    param($LiteralPath, $Algorithm)
    $script:state.hashes++
    if ($LiteralPath -cne $script:state.path -or $Algorithm -cne 'SHA256') { throw 'Download verification changed its path or digest algorithm.' }
    if ($null -ne $script:state.hashFailure) { throw $script:state.hashFailure }
    return Microsoft.PowerShell.Utility\Get-FileHash -LiteralPath $LiteralPath -Algorithm $Algorithm
}

function Invoke-TestDownload {
    param(
        [string]$Name, [object[]]$Steps, [int]$Calls, [int]$Sleeps, [int]$Hashes,
        [Exception]$ExpectedFailure, [switch]$DigestFailure, [Exception]$HashFailure
    )
    $script:state = @{
        path = Join-Path $Directory "$Name.bin"; steps = $Steps; calls = 0; sleeps = 0; hashes = 0; hashFailure = $HashFailure
    }
    $failure = $null
    try { Save-P4kxPinnedDownload $script:url $script:expectedDigest $script:state.path }
    catch { $failure = $_ }
    if ($script:state.calls -ne $Calls -or $script:state.sleeps -ne $Sleeps -or $script:state.hashes -ne $Hashes) {
        throw "Unexpected download attempt/delay/hash counts for $Name."
    }
    if ($null -ne $ExpectedFailure) {
        if ($null -eq $failure -or -not [object]::ReferenceEquals($failure.Exception, $ExpectedFailure)) {
            throw "Download did not preserve the original terminal exception for $Name."
        }
    }
    elseif ($DigestFailure) {
        if ($null -eq $failure -or $failure.Exception.Message -cne "Pinned source/tool digest mismatch: $script:url") {
            throw "Incorrect pinned bytes were accepted or misdiagnosed for $Name."
        }
    }
    else {
        if ($null -ne $failure) { throw $failure }
        $actual = [IO.File]::ReadAllBytes($script:state.path)
        if ([Convert]::ToHexString($actual) -cne [Convert]::ToHexString($script:payload)) {
            throw "Partial download bytes survived successful replacement for $Name."
        }
    }
    $script:caseCount++
}

function New-TestStep {
    param([Exception]$Failure, [switch]$WrongBytes)
    return @{failure = $Failure; bytes = if ($null -ne $Failure -or $WrongBytes) { $script:partial } else { $script:payload }}
}

try {
    Invoke-TestDownload 'first-success' @((New-TestStep)) 1 0 1
    foreach ($status in @(502, 503, 504)) {
        Invoke-TestDownload "http-$status-recovered" @(
            (New-TestStep (New-TestHttpFailure $status)), (New-TestStep (New-TestHttpFailure $status)), (New-TestStep)
        ) 3 2 1
    }
    foreach ($status in @(400, 401, 403, 404, 408, 429, 500, 501, 505)) {
        $failure = New-TestHttpFailure $status
        Invoke-TestDownload "http-$status-fatal" @((New-TestStep $failure)) 1 0 0 -ExpectedFailure $failure
    }
    $exhausted = New-TestHttpFailure 504
    Invoke-TestDownload 'http-504-exhausted' @(
        (New-TestStep (New-TestHttpFailure 504)), (New-TestStep (New-TestHttpFailure 504)), (New-TestStep $exhausted)
    ) 3 2 0 -ExpectedFailure $exhausted
    $permanent = New-TestHttpFailure 404
    Invoke-TestDownload 'http-transient-then-permanent' @(
        (New-TestStep (New-TestHttpFailure 504)), (New-TestStep $permanent)
    ) 2 1 0 -ExpectedFailure $permanent
    $baseGateway = [Net.Http.HttpRequestException]::new('synthetic base HTTP failure', $null, [Net.HttpStatusCode]::BadGateway)
    Invoke-TestDownload 'base-http-502-recovered' @((New-TestStep $baseGateway), (New-TestStep)) 2 1 1
    $baseMissing = [Net.Http.HttpRequestException]::new('synthetic base HTTP failure', $null, [Net.HttpStatusCode]::NotFound)
    Invoke-TestDownload 'base-http-404-fatal' @((New-TestStep $baseMissing)) 1 0 0 -ExpectedFailure $baseMissing
    foreach ($reason in @(
        [Net.Sockets.SocketError]::ConnectionReset, [Net.Sockets.SocketError]::ConnectionAborted,
        [Net.Sockets.SocketError]::TimedOut, [Net.Sockets.SocketError]::TryAgain
    )) {
        Invoke-TestDownload "network-$reason-recovered" @((New-TestStep (New-TestNetworkFailure $reason)), (New-TestStep)) 2 1 1
    }
    $timeout = [OperationCanceledException]::new('synthetic request timeout', [TimeoutException]::new('synthetic deadline'))
    Invoke-TestDownload 'request-timeout-recovered' @((New-TestStep $timeout), (New-TestStep)) 2 1 1
    $fatal = [ordered]@{
        'dns-host-missing' = New-TestNetworkFailure ([Net.Sockets.SocketError]::HostNotFound)
        'tls' = [Net.Http.HttpRequestException]::new('synthetic TLS failure', [Security.Authentication.AuthenticationException]::new('synthetic certificate refusal'))
        'request-unknown' = [Net.Http.HttpRequestException]::new('synthetic unclassified request failure')
        'cancelled' = [OperationCanceledException]::new('synthetic caller cancellation')
        'local-io' = [IO.IOException]::new('synthetic local file failure')
        'local-access' = [UnauthorizedAccessException]::new('synthetic local file access refusal')
        'misleading-text' = [InvalidOperationException]::new('504 Gateway Time-out')
    }
    foreach ($name in $fatal.Keys) {
        Invoke-TestDownload $name @((New-TestStep $fatal[$name])) 1 0 0 -ExpectedFailure $fatal[$name]
    }
    Invoke-TestDownload 'wrong-digest-first' @((New-TestStep -WrongBytes)) 1 0 1 -DigestFailure
    Invoke-TestDownload 'wrong-digest-after-retry' @((New-TestStep (New-TestHttpFailure 504)), (New-TestStep -WrongBytes)) 2 1 1 -DigestFailure
    $hashFailure = [IO.IOException]::new('synthetic hash-read failure')
    Invoke-TestDownload 'hash-read-failed' @((New-TestStep)) 1 0 1 -HashFailure $hashFailure -ExpectedFailure $hashFailure
    "PASS: $script:caseCount pinned download retry, terminal error, partial overwrite and digest cases; no network, tool or native executable invoked."
}
finally {
    foreach ($response in $script:responses) { $response.Dispose() }
}
