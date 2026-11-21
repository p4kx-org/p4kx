param([Parameter(Mandatory)][string]$RequestPath)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-detached-control.ps1')
$request = Read-P4kxDetachedReceipt $RequestPath
Assert-P4kxDetachedBinding $request (Split-Path -Parent $RequestPath)
$directory = $request.directory
$deadline = ConvertFrom-P4kxMsiBudgetTimestamp $request.worker_deadline_utc
$diagnostics = Join-Path $directory 'diagnostics'
$context = New-P4kxMsiPhaseContext $diagnostics $request.target
$worker = Start-P4kxMsiPhase $context 'worker'
$waiting = Start-P4kxMsiPhase $context 'fixture/cross-step-wait'
$process = Get-Process -Id $PID
try {
    Write-P4kxDetachedReceipt (Join-Path $directory 'worker-start.json') @{
        schema=1;role='native-msi-detached-control-worker';pid=$PID;started_utc_ticks=$process.StartTime.ToUniversalTime().Ticks;
        request_sha256=(Get-P4kxDetachedDigest $RequestPath);tracking_sha256=(Get-P4kxDetachedTrackingDigest);
        process_arch=[Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString();completed_success=$false;
        native_completion_unproven=$true;mutation_authority=$false
    }
} finally { $process.Dispose() }
$deadlineExpired = $false
try {
    while (-not [IO.File]::Exists((Join-Path $directory 'release.json'))) {
        if ([DateTimeOffset]::UtcNow -ge $deadline) { $deadlineExpired = $true; throw 'Harmless detached control deadline reached before release.' }
        Start-Sleep -Milliseconds 25
    }
    $release = Read-P4kxDetachedReceipt (Join-Path $directory 'release.json')
    if ($release.request_sha256 -cne (Get-P4kxDetachedDigest $RequestPath)) { throw 'Detached control release belongs to another request.' }
    Stop-P4kxMsiPhase $context $waiting
    Stop-P4kxMsiPhase $context $worker
    Write-P4kxDetachedReceipt (Join-Path $directory 'worker-result.json') @{
        schema=1;role='native-msi-detached-control-worker-result';request_sha256=(Get-P4kxDetachedDigest $RequestPath);
        released=$true;control_passed=$true;completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
    }
} catch {
    Write-P4kxDetachedReceipt (Join-Path $directory 'worker-error.json') @{
        schema=1;role='native-msi-detached-control-worker-error';request_sha256=(Get-P4kxDetachedDigest $RequestPath);
        deadline_expired=$deadlineExpired;completed_success=$false;native_completion_unproven=$true;mutation_authority=$false
    }
    Stop-P4kxMsiPhase $context $waiting -Failure $_.Exception
    Stop-P4kxMsiPhase $context $worker -Failure $_.Exception
    throw
}
