param([Parameter(Mandatory)][string]$RequestPath)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-observer-request.ps1')
$request = Read-P4kxDetachedReceipt $RequestPath
Assert-P4kxMsiObserverBinding $request (Split-Path -Parent $RequestPath)
Set-Location -LiteralPath $request.location
[IO.Directory]::SetCurrentDirectory($request.native_directory)
$parameters = Read-P4kxDetachedReceipt $request.invocation
$process = Get-Process -Id $PID
try {
    $binding = @{schema=1;role='native-msi-observer-parent-start';pid=$PID;started_utc_ticks=$process.StartTime.ToUniversalTime().Ticks;
        request_sha256=(Get-P4kxDetachedDigest $RequestPath);tracking_sha256=(Get-P4kxDetachedTrackingDigest);mode=$request.mode;
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false}
    Write-P4kxDetachedReceipt (Join-Path $request.directory 'parent-start.json') $binding
} finally { $process.Dispose() }
$failure = $null
try {
    if ($request.mode -ceq 'Lifecycle') { & (Join-Path $PSScriptRoot 'check-msi.ps1') @parameters }
    else { Invoke-P4kxMsiDiagnosticWorker (Join-Path $PSScriptRoot $request.script) $parameters }
} catch { $failure = $_.Exception }
$finished = $binding.Clone()
$finished.role='native-msi-observer-parent-result'
$finished.returned_utc=[DateTimeOffset]::UtcNow.ToString('o')
$finished.invocation_succeeded=($null -eq $failure)
$finished.failure_type=$(if($null -ne $failure){$failure.GetType().FullName}else{$null})
Write-P4kxDetachedReceipt (Join-Path $request.directory 'parent-result.json') $finished
if ($null -ne $failure) { throw $failure }
