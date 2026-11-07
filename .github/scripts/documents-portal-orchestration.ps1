$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'documents-portal-session.ps1')
. (Join-Path $PSScriptRoot 'documents-portal-identity.ps1')

function Invoke-P4kxCheckedDocumentsSession {
    param([string]$Script, [string[]]$Arguments, [string]$Directory, [string]$Name,
        [hashtable]$Environment, [string]$Config, [string]$MountPoint,
        [Collections.Generic.List[Exception]]$Failures)
    try {
        $result = Invoke-P4kxDocumentsSession $Script $Arguments $Directory $Name $Environment $Config
        if ($result.exit_code -ne 0) { $Failures.Add([InvalidOperationException]::new("Native $Name session exited $($result.exit_code); inspect its bounded logs.")) }
    }
    catch { $Failures.Add($_.Exception) }
    try { Assert-P4kxDocumentsSessionEnded $Directory $MountPoint; return $true }
    catch { $Failures.Add($_.Exception) }
    $proof = Join-Path $Directory 'session-services.json'
    if (Test-Path -LiteralPath $proof) {
        try {
            $null = Invoke-P4kxPortalMeasurement python3 @('-B', (Join-Path $PSScriptRoot 'stop-owned-portals.py'), $proof) $Directory 'owned-cleanup' -Milliseconds 15000
        }
        catch { $Failures.Add($_.Exception) }
    }
    return $false
}

function Assert-P4kxDocumentsMountAbsent {
    param([string]$MountPoint, [string]$Directory)
    $mount = Invoke-P4kxPortalProcess findmnt @('--kernel', '--json', '--mountpoint', $MountPoint) $Directory 'initial-mount'
    if ($mount.exit_code -ne 1 -or $mount.stdout.Trim() -or $mount.stderr.Trim()) { throw 'A preexisting or unmeasurable Documents mount prevents owned-session verification.' }
}

function Invoke-P4kxLinuxDocumentsPhases {
    param($Context, $Services)
    $failures = [Collections.Generic.List[Exception]]::new()
    $mountPoint = Join-Path $env:XDG_RUNTIME_DIR 'doc'
    Assert-P4kxDocumentsMountAbsent $mountPoint $Context.output
    $stock = Join-Path $Context.output 'stock'
    $environment = Get-P4kxDocumentsSessionEnvironment $stock $env:FLATPAK_USER_DIR
    $arguments = @('-Target', $Context.target, '-Version', $Context.version, '-AssetDirectory', $Context.assets,
        '-FixtureDirectory', $Context.fixture, '-BuildMetadata', $Context.metadata, '-OutputDirectory', $stock)
    $ended = Invoke-P4kxCheckedDocumentsSession (Join-Path $PSScriptRoot 'check-linux-stock-session.ps1') $arguments $stock 'stock' $environment '' $mountPoint $failures
    if (-not $ended) { throw [AggregateException]::new('Stock session teardown failed; no further bus may start.', $failures) }
    $positive = Join-Path $Context.output 'api5'
    try {
        $environment = Get-P4kxDocumentsSessionEnvironment $positive $env:FLATPAK_USER_DIR
        $config = New-P4kxDocumentsSessionConfig (Join-Path $positive 'activation') $Services
        $arguments = @('-Mode', 'Api5', '-Target', $Context.target, '-Version', $Context.version, '-FixtureDirectory', $Context.fixture,
            '-StatePath', (Join-Path $stock 'flatpak-state.json'), '-ServiceBuildMetadata', $Context.services, '-OutputDirectory', $positive)
        $ended = $false
        $ended = Invoke-P4kxCheckedDocumentsSession (Join-Path $PSScriptRoot 'check-flatpak-session.ps1') $arguments $positive 'api5' $environment $config $mountPoint $failures
    }
    catch { $failures.Add($_.Exception) }
    if (-not $ended) { throw [AggregateException]::new('API5 teardown failed; no cleanup bus may start.', $failures) }
    try { Complete-P4kxDocumentsCleanup $Context $Services $stock $positive $failures }
    catch { $failures.Add($_.Exception) }
    if ($failures.Count -ne 0) { throw [AggregateException]::new('Native main-session checks failed; cleanup is not runtime proof.', $failures) }
    return @{stock=$stock; positive=$positive}
}

function Complete-P4kxDocumentsCleanup {
    param($Context, $Services, [string]$Stock, [string]$Positive, [Collections.Generic.List[Exception]]$Failures)
    $finished = Join-Path $Positive 'package-cleanup.json'
    if (Test-Path -LiteralPath $finished) {
        $proof = Get-Content -LiteralPath $finished -Raw | ConvertFrom-Json
        if ($proof.outcome -ceq 'success' -and $proof.runtime_pass -eq $false) { return }
    }
    $cleanup = Join-Path $Context.output 'cleanup-only'
    $environment = Get-P4kxDocumentsSessionEnvironment $cleanup $env:FLATPAK_USER_DIR
    $config = New-P4kxDocumentsSessionConfig (Join-Path $cleanup 'activation') $Services
    $arguments = @('-Mode', 'Cleanup', '-Target', $Context.target, '-Version', $Context.version, '-FixtureDirectory', $Context.fixture,
        '-StatePath', (Join-Path $Stock 'flatpak-state.json'), '-ServiceBuildMetadata', $Context.services, '-OutputDirectory', $cleanup)
    $null = Invoke-P4kxCheckedDocumentsSession (Join-Path $PSScriptRoot 'check-flatpak-session.ps1') $arguments $cleanup 'cleanup-only' $environment $config (Join-Path $env:XDG_RUNTIME_DIR 'doc') $Failures
}
