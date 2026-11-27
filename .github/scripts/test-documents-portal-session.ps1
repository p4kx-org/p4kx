<#
.SYNOPSIS
Checks private activation and run-all/teardown ordering with pure process substitutes.
#>
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'documents-portal-orchestration.ps1')
if (Test-Path -LiteralPath $Directory) { throw 'Session contract directory must be new.' }
New-Item -ItemType Directory -Path $Directory | Out-Null
$listener = 'p4kx-docs-' + ('a' * 32)
$xml = [xml](ConvertTo-P4kxDocumentsSessionConfig $listener '/owned/a&b/services')
if ($xml.busconfig.listen -cne "unix:abstract=$listener" -or $xml.busconfig.auth -cne 'EXTERNAL' -or $xml.busconfig.servicedir -cne '/owned/a&b/services') { throw 'Session XML lost escaping, listener or UID authentication.' }
$children = @($xml.busconfig.ChildNodes | ForEach-Object LocalName)
if ([Array]::IndexOf($children, 'servicedir') -ge [Array]::IndexOf($children, 'standard_session_servicedirs')) { throw 'Official source activation does not have first precedence.' }
foreach ($bad in @('p4kx-docs-short', 'unix:anonymous', ('p4kx-docs-' + ('a' * 32) + ',other=value'))) {
    $rejected = $false
    try { ConvertTo-P4kxDocumentsSessionConfig $bad '/owned/services' | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'A foreign or injected private listener was accepted.' }
}
$activation = ConvertTo-P4kxDocumentsActivation 'xdg-document-portal' '/owned/a"b/xdg-document-portal'
if ($activation -cnotmatch 'Exec="/owned/a\\"b/xdg-document-portal"' -or $activation.Contains('SystemdService=')) { throw 'Private activation escaped into the host service manager or lost argument quoting.' }
$realEnd = ${function:Assert-P4kxDocumentsSessionEnded}
$script:events = [Collections.Generic.List[string]]::new()
$script:badExit = ''; $script:badEnd = ''; $script:cleanupWritten = $true; $script:badConfig = $false
function Assert-P4kxDocumentsMountAbsent { param($MountPoint, $Directory) $script:events.Add('absent') }
function Get-P4kxDocumentsSessionEnvironment {
    param($Directory, $FlatpakDirectory)
    New-Item -ItemType Directory -Path $Directory -Force | Out-Null
    return @{}
}
function New-P4kxDocumentsSessionConfig {
    param($Directory, $Build)
    $phase = [IO.Path]::GetFileName([IO.Path]::GetDirectoryName($Directory))
    $script:events.Add("config:$phase")
    if ($script:badConfig -and $phase -ceq 'api5') { throw 'synthetic setup failure' }
    return 'synthetic-session.conf'
}
function Invoke-P4kxDocumentsSession {
    param($Script, $Arguments, $Directory, $Name, $Environment, $Config)
    $script:events.Add("run:$Name")
    if ($Name -ceq 'cleanup-only' -or ($Name -ceq 'api5' -and $script:cleanupWritten)) {
        @{outcome='success'; runtime_pass=$false} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Directory 'package-cleanup.json')
    }
    return @{exit_code=$(if ($Name -ceq $script:badExit) { 37 } else { 0 })}
}
function Assert-P4kxDocumentsSessionEnded {
    param($Directory, $MountPoint)
    $phase = [IO.Path]::GetFileName($Directory)
    $script:events.Add("ended:$phase")
    if ($phase -ceq $script:badEnd) { throw 'synthetic unproved teardown' }
}
function Invoke-P4kxPortalMeasurement { throw 'A mock attempted an unplanned native cleanup command.' }
function Invoke-P4kxPortalProcess { return @{exit_code=1; stdout=''; stderr=''} }
function Get-P4kxPortalProcessStart { return $null }
function Invoke-Scenario {
    param([string]$Name, [switch]$Failure)
    $script:events.Clear()
    $output = Join-Path $Directory $Name
    New-Item -ItemType Directory -Path $output | Out-Null
    $context = @{target='synthetic'; version='0.5.0'; output=$output; assets='synthetic'; fixture='synthetic'; metadata='synthetic'; services='synthetic'}
    $errorRecord = $null
    try { Invoke-P4kxLinuxDocumentsPhases $context @{} | Out-Null } catch { $errorRecord = $_.Exception }
    if ($Failure.IsPresent -ne ($null -ne $errorRecord)) { throw 'Session scenario had an unexpected success/failure outcome.' }
    return @($script:events)
}
$beforeRuntime = $env:XDG_RUNTIME_DIR
$beforeFlatpak = $env:FLATPAK_USER_DIR
try {
    $env:XDG_RUNTIME_DIR = '/run/user/1000'; $env:FLATPAK_USER_DIR = 'synthetic'
    $trace = Invoke-Scenario 'success'
    if (($trace -join ',') -cne 'absent,run:stock,ended:stock,config:api5,run:api5,ended:api5') { throw 'Main sessions ran out of order or reused cleanup as a runtime pass.' }
    $script:badExit = 'stock'
    $trace = Invoke-Scenario 'negative-failure' -Failure
    if ('run:api5' -cnotin $trace) { throw 'A measured negative failure hid the independent positive after proved teardown.' }
    $script:badExit = ''; $script:badEnd = 'stock'
    $trace = Invoke-Scenario 'stock-teardown' -Failure
    if ('run:api5' -cin $trace -or 'run:cleanup-only' -cin $trace) { throw 'A new bus started after unproved stock teardown.' }
    $script:badEnd = ''; $script:badExit = 'api5'; $script:cleanupWritten = $false
    $trace = Invoke-Scenario 'positive-failure' -Failure
    if ('run:cleanup-only' -cnotin $trace) { throw 'Failed positive phase did not perform its separately labeled owned cleanup.' }
    $script:badEnd = 'api5'
    $trace = Invoke-Scenario 'positive-teardown' -Failure
    if ('run:cleanup-only' -cin $trace) { throw 'Cleanup bus started after unproved positive teardown.' }
    $script:badEnd = ''; $script:badExit = ''; $script:badConfig = $true
    $trace = Invoke-Scenario 'setup-failure' -Failure
    if ('run:api5' -cin $trace -or 'run:cleanup-only' -cnotin $trace) { throw 'Setup failure was hidden or lost its safe cleanup-only path.' }
    $proofDir = Join-Path $Directory 'real-teardown-contract'
    New-Item -ItemType Directory -Path $proofDir | Out-Null
    @{stage='initial'; inventory_complete=$true; services=@(); session='synthetic'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $proofDir 'session-services.json')
    $rejected = $false
    try { & $realEnd $proofDir 'synthetic' } catch { $rejected = $true }
    if (-not $rejected) { throw 'An initial-only process inventory was promoted to final teardown proof.' }
    $finalDir = Join-Path (Join-Path $Directory 'final-controller-contract') 'stock'
    New-Item -ItemType Directory -Path $finalDir -Force | Out-Null
    @{stage='final'; inventory_complete=$true; services=@(); session='synthetic'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $finalDir 'session-services.json')
    $rejected = $false
    try { & $realEnd $finalDir 'synthetic' } catch { $rejected = $true }
    if (-not $rejected) { throw 'Missing controller proof allowed another session.' }
    @{schema=1; role='stock'; session='synthetic'; direct_processes_exited=$true; signal_contract='direct-process-only';
        bus=@{started=$true; exited=$true; pid=100; exit_code=137; signal_contract='direct-process-only'};
        worker=@{started=$true; exited=$true; pid=101; exit_code=0; signal_contract='direct-process-only'}} |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $finalDir 'session-controller.json')
    & $realEnd $finalDir 'synthetic'
    $end = Get-Content -LiteralPath (Join-Path $finalDir 'teardown.json') -Raw | ConvertFrom-Json
    if ($end.controller_processes_exited -cne 'success') { throw 'Final teardown omitted its direct controller proof.' }
}
finally { $env:XDG_RUNTIME_DIR = $beforeRuntime; $env:FLATPAK_USER_DIR = $beforeFlatpak }
'PASS: private config/quoting and six pure session-order/failure/cleanup scenarios; no bus, daemon, mount or random generation.'
