$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'msi-diagnostic-budget.ps1')
. (Join-Path $PSScriptRoot 'msi-diagnostic-phase.ps1')

function Get-P4kxDetachedDigest([string]$Path) {
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Assert-P4kxDetachedReceiptLength([long]$Length) {
    if ($Length -gt 16KB) { throw 'Detached control receipt exceeds its metadata bound.' }
}

function Get-P4kxDetachedReceiptDigest([string]$Path) {
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        $length = $stream.Length
        Assert-P4kxDetachedReceiptLength $length
        $bytes = [byte[]]::new([int]$length)
        $stream.ReadExactly($bytes, 0, $bytes.Length)
        if ($stream.ReadByte() -ne -1) { throw 'Detached control receipt changed while it was read.' }
        return [Convert]::ToHexStringLower([Security.Cryptography.SHA256]::HashData($bytes))
    } finally { $stream.Dispose() }
}

function Get-P4kxDetachedTrackingDigest {
    if ([string]::IsNullOrEmpty($env:RUNNER_TRACKING_ID)) { return $null }
    $bytes = [Text.Encoding]::UTF8.GetBytes($env:RUNNER_TRACKING_ID)
    return [Convert]::ToHexStringLower([Security.Cryptography.SHA256]::HashData($bytes))
}

function Write-P4kxDetachedReceipt([string]$Path, $Value) {
    $bytes = [Text.UTF8Encoding]::new($false, $true).GetBytes(($Value | ConvertTo-Json -Depth 10 -Compress)+[char]10)
    $destination = [IO.Path]::GetFullPath($Path)
    $directory = [IO.Path]::GetDirectoryName($destination)
    $temporary = Join-Path $directory ('.receipt-' + [IO.Path]::GetRandomFileName() + '.tmp')
    $stream = [IO.File]::Open($temporary, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    try {
        try { $stream.Write($bytes); $stream.Flush($true) } finally { $stream.Dispose() }
        [IO.File]::Move($temporary,$destination,$false)
    } finally {
        if ([IO.File]::Exists($temporary)) { [IO.File]::Delete($temporary) }
    }
}

function Read-P4kxDetachedReceipt([string]$Path) {
    Assert-P4kxDetachedReceiptLength (Get-Item -LiteralPath $Path).Length
    return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json -AsHashtable -Depth 10 -DateKind String
}

function Get-P4kxDetachedControlSources {
    foreach ($name in @('msi-detached-control.ps1','msi-detached-control-worker.ps1','invoke-msi-detached-control.ps1','platform-contract.ps1','msi-diagnostic-budget.ps1','msi-diagnostic-phase.ps1','msi-diagnostic-utf8.ps1')) {
        @{name=$name;sha256=(Get-P4kxDetachedDigest (Join-Path $PSScriptRoot $name))}
    }
}

function Assert-P4kxDetachedBinding($Request, [string]$Directory) {
    if ($Request.schema -ne 1 -or $Request.role -cne 'native-msi-detached-control-request' -or
        $Request.revision -cne $env:GITHUB_SHA -or $Request.run -cne $env:GITHUB_RUN_ID -or
        $Request.attempt -cne $env:GITHUB_RUN_ATTEMPT -or $Request.target -cne $env:P4KX_TARGET -or
        $Request.directory -cne [IO.Path]::GetFullPath($Directory) -or
        $Request.deadline_utc -cne $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC -or
        $Request.tracking_sha256 -cne (Get-P4kxDetachedTrackingDigest)) {
        throw 'Detached control source, run, CPU, location, tracking or deadline binding differs.'
    }
    $platform = Get-P4kxPlatform -Target $Request.target
    Assert-P4kxNativeHost $platform
    if ([Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString() -ine $platform.runner_arch) { throw 'Detached control process CPU differs from its native target.' }
    $expected = @(Get-P4kxDetachedControlSources)
    $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    if ($Request.sources.Count -ne $expected.Count) { throw 'Detached control source inventory is incomplete.' }
    foreach ($source in $Request.sources) {
        if (-not $names.Add($source.name)) { throw 'Detached control source inventory repeats a name.' }
        $matched = @($expected | Where-Object { $_.name -ceq $source.name -and $_.sha256 -ceq $source.sha256 })
        if ($matched.Count -ne 1) {
            throw 'Detached control source bytes differ.'
        }
    }
}

function Open-P4kxDetachedOwnedProcess {
    param($Handle, $Request, [ValidateSet('native-msi-detached-control-handle','native-msi-observer-handle')][string]$Role='native-msi-detached-control-handle')
    if ($Handle.schema -ne 1 -or $Handle.role -cne $Role -or
        $Handle.request_sha256 -cne (Get-P4kxDetachedDigest (Join-Path $Request.directory 'request.json'))) {
        throw 'Detached control handle is not bound to its original request.'
    }
    $process = Get-Process -Id $Handle.pid -ErrorAction Stop
    try {
        if ($process.StartTime.ToUniversalTime().Ticks -ne $Handle.started_utc_ticks -or
            -not [string]::Equals($process.Path, $Handle.executable, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Detached control PID was reused or executable identity differs.'
        }
        [void]$process.Handle
        return $process
    } catch { $process.Dispose(); throw }
}

function New-P4kxDetachedControlRequest([string]$Directory, [int]$Milliseconds) {
    if (-not $IsWindows -or $Milliseconds -le 0 -or $Milliseconds -gt 90000) { throw 'Invalid harmless native control allowance.' }
    $row = Get-P4kxPlatform -Target $env:P4KX_TARGET
    Assert-P4kxNativeHost $row
    if ([Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString() -ine $row.runner_arch) { throw 'Detached control launcher must run on its native CPU.' }
    $tracking = Get-P4kxDetachedTrackingDigest
    if ($env:GITHUB_ACTIONS -ceq 'true' -and ($env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or $null -eq $tracking)) {
        throw 'Native CI control requires the original hosted-runner process tracking tag.'
    }
    if ([IO.Path]::Exists($Directory)) { throw 'Detached control directory must be new.' }
    $budget = [Math]::Min($Milliseconds, (Get-P4kxMsiWorkerBudget $env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC))
    $sources = @(Get-P4kxDetachedControlSources)
    New-Item -ItemType Directory -Path $Directory | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $Directory 'diagnostics') | Out-Null
    return @{schema=1;role='native-msi-detached-control-request';revision=$env:GITHUB_SHA;run=$env:GITHUB_RUN_ID;attempt=$env:GITHUB_RUN_ATTEMPT;
        target=$row.target;directory=[IO.Path]::GetFullPath($Directory);deadline_utc=$env:P4KX_MSI_DIAGNOSTIC_DEADLINE_UTC;
        worker_deadline_utc=[DateTimeOffset]::UtcNow.AddMilliseconds($budget).ToString('o');worker_budget_ms=$budget;
        tracking_sha256=$tracking;powershell_version=$PSVersionTable.PSVersion.ToString();hosted_runner=($env:GITHUB_ACTIONS -ceq 'true');sources=$sources;
        completed_success=$false;native_completion_unproven=$true;mutation_authority=$false}
}
