# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

$ErrorActionPreference = 'Stop'

function Test-P4kxTransientDownloadFailure {
    param([Exception]$Failure)
    $transientStatus = @([Net.HttpStatusCode]::BadGateway, [Net.HttpStatusCode]::ServiceUnavailable, [Net.HttpStatusCode]::GatewayTimeout)
    if ($Failure -is [Microsoft.PowerShell.Commands.HttpResponseException]) {
        return $Failure.Response.StatusCode -in $transientStatus
    }
    if ($Failure -is [Net.Http.HttpRequestException]) {
        if ($null -ne $Failure.StatusCode) { return $Failure.StatusCode -in $transientStatus }
        for ($cause = $Failure.InnerException; $null -ne $cause; $cause = $cause.InnerException) {
            if ($cause -is [Security.Authentication.AuthenticationException]) { return $false }
            if ($cause -is [Net.Sockets.SocketException]) {
                return $cause.SocketErrorCode -in @(
                    [Net.Sockets.SocketError]::ConnectionReset,
                    [Net.Sockets.SocketError]::ConnectionAborted,
                    [Net.Sockets.SocketError]::TimedOut,
                    [Net.Sockets.SocketError]::TryAgain
                )
            }
        }
        return $false
    }
    return $Failure -is [OperationCanceledException] -and $Failure.InnerException -is [TimeoutException]
}

function Save-P4kxPinnedDownload {
    param([string]$Url, [string]$Digest, [string]$Destination)
    $maximumAttempts = 3
    $retryDelaySeconds = 2
    for ($attempt = 1; ; $attempt++) {
        try {
            Invoke-WebRequest -Uri $Url -OutFile $Destination -MaximumRetryCount 0 -ErrorAction Stop
            break
        }
        catch {
            if ($attempt -ge $maximumAttempts -or -not (Test-P4kxTransientDownloadFailure -Failure $_.Exception)) { throw }
            Write-Warning "Temporary pinned download failure; retrying $($attempt + 1)/${maximumAttempts}: $Url"
            Start-Sleep -Seconds $retryDelaySeconds
        }
    }
    if ((Get-FileHash -LiteralPath $Destination -Algorithm SHA256).Hash.ToLowerInvariant() -cne $Digest) {
        throw "Pinned source/tool digest mismatch: $Url"
    }
}
