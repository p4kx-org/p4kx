<#
.SYNOPSIS
Checks direct session-process ownership with toy PowerShell children and no D-Bus or services.
#>
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'documents-session-controller.ps1')
. (Join-Path $PSScriptRoot 'documents-session-proof.ps1')
if (Test-Path -LiteralPath $Directory) { throw 'Controller contract directory must be fresh.' }
New-Item -ItemType Directory -Path $Directory | Out-Null
$script:actualProcess = (Get-Item Function:New-P4kxPackageProcess).ScriptBlock
$script:pwsh = (Get-Process -Id $PID).Path
$script:environmentKeys = @('XDG_DATA_HOME', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'FLATPAK_USER_DIR',
    'DBUS_STARTER_ADDRESS', 'DBUS_STARTER_BUS_TYPE', 'DBUS_SESSION_BUS_ADDRESS',
    'P4KX_PORTAL_SESSION_ROLE', 'P4KX_PORTAL_PARENT_BUS', 'P4KX_FOLDERS_ROOT')
$script:environmentKeys | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Directory 'environment-keys.json') -Encoding utf8NoBOM
@'
param([string]$Destination)
$ErrorActionPreference = 'Stop'
$observed = [ordered]@{}
foreach ($name in (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'environment-keys.json') -Raw | ConvertFrom-Json)) {
    $observed[$name] = [Environment]::GetEnvironmentVariable($name)
}
$observed | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath $Destination -Encoding utf8NoBOM
'@ | Set-Content -LiteralPath (Join-Path $Directory 'toy-environment.ps1') -Encoding utf8NoBOM
$script:busMode = 'normal'; $script:workerStartFailure = $false; $script:cleanupFailure = $false
$script:actualClose = (Get-Item Function:Close-P4kxDocumentsProcess).ScriptBlock
$script:started = [Collections.Generic.List[string]]::new()
$script:address = 'unix:abstract=p4kx-docs-' + ('a' * 32) + ',guid=' + ('b' * 32)
$busScript = Join-Path $Directory 'toy-bus.ps1'
@'
param([string]$Mode, [string]$Directory, [string]$Address)
$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'toy-environment.ps1') -Destination (Join-Path $Directory 'bus-environment.json')
switch ($Mode) {
    silent { Start-Sleep -Seconds 20; exit 0 }
    failure { [Console]::Error.Write('synthetic bus failed'); exit 42 }
    invalid { [Console]::Out.WriteLine('tcp:host=127.0.0.1,port=12345') }
    duplicate { [Console]::Out.WriteLine($Address); [Console]::Out.WriteLine($Address) }
    excessive { [Console]::Error.Write('x' * 140000) }
    default { [Console]::Out.WriteLine($Address) }
}
if ($Mode -ceq 'early-exit') {
    while (-not (Test-Path -LiteralPath (Join-Path $Directory 'worker-started'))) { Start-Sleep -Milliseconds 10 }
    exit 27
}
Start-Sleep -Seconds 20
'@ | Set-Content -LiteralPath $busScript -Encoding utf8NoBOM
$workerScript = Join-Path $Directory 'toy-worker.ps1'
@'
param([string]$Mode)
$ErrorActionPreference = 'Stop'
$directory = $env:P4KX_FOLDERS_ROOT
& (Join-Path $PSScriptRoot 'toy-environment.ps1') -Destination (Join-Path $directory 'worker-environment.json')
[IO.File]::WriteAllText((Join-Path $directory 'worker-started'), 'started')
switch ($Mode) {
    timeout { Start-Sleep -Seconds 20 }
    excessive { [Console]::Out.Write('x' * 20000) }
    failure { [Console]::Error.Write('synthetic worker failed'); exit 37 }
    default { [Console]::Out.Write('synthetic stdout'); [Console]::Error.Write('synthetic stderr') }
}
'@ | Set-Content -LiteralPath $workerScript -Encoding utf8NoBOM
$script:busScript = $busScript
function New-P4kxPackageProcess {
    param($Executable, $Arguments, $Directory)
    $script:started.Add($Executable)
    if ($Executable -ceq 'dbus-daemon') {
        if ($Arguments.Count -ne 3 -or $Arguments[0] -cne '--nofork' -or $Arguments[1] -cne '--print-address=1' -or
            ($Arguments[2] -cne '--session' -and -not $Arguments[2].StartsWith('--config-file=', [StringComparison]::Ordinal))) {
            throw 'Bus launch lost its direct nofork/readiness/selected configuration contract.'
        }
        return & $script:actualProcess $script:pwsh @('-NoProfile', '-File', $script:busScript, '-Mode', $script:busMode, '-Directory', $Directory, '-Address', $script:address) $Directory
    }
    if ($Executable -cne 'pwsh') { throw 'Controller attempted an unowned executable.' }
    $process = & $script:actualProcess $script:pwsh $Arguments $Directory
    if ($script:workerStartFailure) { $process.StartInfo.FileName = Join-Path $Directory 'missing-executable' }
    return $process
}
function Close-P4kxDocumentsProcess {
    param($Owned, $Directory, $Name, $Failures)
    & $script:actualClose $Owned $Directory $Name $Failures
    if ($script:cleanupFailure -and $Name -ceq 'stock-worker') { $Failures.Add([InvalidOperationException]::new('synthetic direct cleanup failure')) }
}
function Assert-ToyEnvironment {
    param($Bus, $Worker, $Owned)
    foreach ($name in @('XDG_DATA_HOME', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'FLATPAK_USER_DIR')) {
        if ($Bus.$name -cne $Owned[$name] -or $Worker.$name -cne $Owned[$name]) { throw 'Bus and worker lost their identical isolated environment.' }
    }
    foreach ($captured in @($Bus, $Worker)) {
        if ((@($captured.PSObject.Properties.Name | Sort-Object) -join ',') -cne
            (@($script:environmentKeys | Sort-Object) -join ',')) {
            throw 'Toy process recorded unrelated environment variables.'
        }
        if ($captured.DBUS_STARTER_ADDRESS -or $captured.DBUS_STARTER_BUS_TYPE -or
            $captured.P4KX_PORTAL_SESSION_ROLE -cne 'stock' -or $captured.P4KX_PORTAL_PARENT_BUS -cne 'unix:abstract=untrusted-parent') {
            throw 'Session inherited a starter route or lost its role/parent marker.'
        }
    }
    if ($Bus.DBUS_SESSION_BUS_ADDRESS -or $Worker.DBUS_SESSION_BUS_ADDRESS -cne $script:address) { throw 'Worker did not use exactly the newly measured bus address.' }
}
function Invoke-ToyScenario {
    param([string]$Name, [string]$BusMode = 'normal', [string]$WorkerMode = 'success', [string]$ExpectedFailure = '', [string]$Config = '')
    $script:busMode = $BusMode; $script:started.Clear()
    $output = Join-Path $Directory $Name
    New-Item -ItemType Directory -Path $output | Out-Null
    $owned = @{XDG_DATA_HOME='owned-data'; XDG_CONFIG_HOME='owned-config'; XDG_CACHE_HOME='owned-cache'; FLATPAK_USER_DIR='owned-flatpak'}
    $errorRecord = $null; $result = $null
    try {
        $result = Invoke-P4kxOwnedDocumentsProcesses $workerScript @('-Mode', $WorkerMode) $output 'stock' $owned $Config -Milliseconds 3000 -ReadinessMilliseconds 1500 -MaximumBytes 4096
    }
    catch { $errorRecord = $_.Exception }
    $script:lastFailure = $errorRecord
    if ($ExpectedFailure) {
        if ($null -eq $errorRecord) { throw 'Toy controller unexpectedly succeeded.' }
        if ($errorRecord.ToString() -cnotlike "*$ExpectedFailure*") { throw [AggregateException]::new('Toy controller failure lost the expected boundary.', $errorRecord) }
    }
    elseif ($null -ne $errorRecord) { throw $errorRecord }
    $proof = Get-Content -LiteralPath (Join-Path $output 'session-controller.json') -Raw | ConvertFrom-Json
    foreach ($process in @($proof.bus, $proof.worker)) {
        if ($process.started -eq $true -and ($process.exited -ne $true -or (Get-Process -Id $process.pid -ErrorAction SilentlyContinue))) {
            throw 'Controller left a directly owned toy process alive.'
        }
    }
    if ($script:started.Count -gt 2 -or $script:started[0] -cne 'dbus-daemon') { throw 'Controller did not retain the two direct process boundary.' }
    if (-not $ExpectedFailure) {
        Assert-P4kxDocumentsControllerEvidence $proof $script:address 'stock'
        $bus = Get-Content -LiteralPath (Join-Path $output 'bus-environment.json') -Raw | ConvertFrom-Json
        $worker = Get-Content -LiteralPath (Join-Path $output 'worker-environment.json') -Raw | ConvertFrom-Json
        Assert-ToyEnvironment $bus $worker $owned
    }
    return $result
}
$names = @('DBUS_SESSION_BUS_ADDRESS', 'DBUS_STARTER_ADDRESS', 'DBUS_STARTER_BUS_TYPE')
$before = @{}
foreach ($name in $names) { $before[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:DBUS_SESSION_BUS_ADDRESS = 'unix:abstract=untrusted-parent'
    $env:DBUS_STARTER_ADDRESS = 'unix:abstract=untrusted-starter'; $env:DBUS_STARTER_BUS_TYPE = 'system'
    $result = Invoke-ToyScenario 'normal'
    if ($result.exit_code -ne 0 -or $result.stdout -cne 'synthetic stdout' -or $result.stderr -cne 'synthetic stderr') { throw 'Controller mixed stdout/stderr or invented exit success.' }
    $result = Invoke-ToyScenario 'worker-failure' -WorkerMode failure
    if ($result.exit_code -ne 37) { throw 'Controller swallowed the actual worker failure.' }
    $null = Invoke-ToyScenario 'readiness-timeout' -BusMode silent -ExpectedFailure 'address timed out'
    $null = Invoke-ToyScenario 'readiness-exit' -BusMode failure -ExpectedFailure 'before worker startup'
    $null = Invoke-ToyScenario 'invalid-address' -BusMode invalid -ExpectedFailure 'exactly one bounded Unix address'
    $null = Invoke-ToyScenario 'duplicate-address' -BusMode duplicate -ExpectedFailure 'exactly one bounded Unix address'
    $null = Invoke-ToyScenario 'bus-output-limit' -BusMode excessive -ExpectedFailure 'output limit'
    $null = Invoke-ToyScenario 'worker-output-limit' -WorkerMode excessive -ExpectedFailure 'output limit'
    $null = Invoke-ToyScenario 'worker-timeout' -WorkerMode timeout -ExpectedFailure 'worker timed out'
    $null = Invoke-ToyScenario 'bus-exit' -BusMode early-exit -WorkerMode timeout -ExpectedFailure 'while its worker'
    $script:workerStartFailure = $true
    $null = Invoke-ToyScenario 'worker-start-failure' -ExpectedFailure 'missing-executable'
    $script:workerStartFailure = $false
    $config = Join-Path $Directory 'private-session.conf'
    '<busconfig><listen>unix:abstract=p4kx-docs-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa</listen></busconfig>' | Set-Content -LiteralPath $config -Encoding utf8NoBOM
    $null = Invoke-ToyScenario 'private-config' -Config $config
    '<busconfig><listen>unix:abstract=p4kx-docs-cccccccccccccccccccccccccccccccc</listen></busconfig>' | Set-Content -LiteralPath $config -Encoding utf8NoBOM
    $null = Invoke-ToyScenario 'private-mismatch' -Config $config -ExpectedFailure 'differs from its owned configuration'
    $previousAddress = $script:address
    $script:address = 'unix:abstract=untrusted-parent'
    $null = Invoke-ToyScenario 'parent-route' -ExpectedFailure 'reused the parent bus'
    $script:address = $previousAddress
    $script:cleanupFailure = $true
    $null = Invoke-ToyScenario 'combined-cleanup-failure' -WorkerMode timeout -ExpectedFailure 'worker timed out'
    if ($script:lastFailure.InnerExceptions.Count -ne 2 -or $script:lastFailure.ToString() -cnotlike '*synthetic direct cleanup failure*') { throw 'Original timeout and direct cleanup failure were not both retained.' }
    $script:cleanupFailure = $false
    if ($env:DBUS_SESSION_BUS_ADDRESS -cne 'unix:abstract=untrusted-parent' -or $env:DBUS_STARTER_ADDRESS -cne 'unix:abstract=untrusted-starter') { throw 'Controller changed its parent environment.' }
}
finally { foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $before[$name]) } }
$source = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'portal-process.ps1') -Raw
$tokens = $null; $errors = $null
$ast = [Management.Automation.Language.Parser]::ParseInput($source, [ref]$tokens, [ref]$errors)
$kills = @($ast.FindAll({ param($node) $node -is [Management.Automation.Language.InvokeMemberExpressionAst] -and $node.Member.Value -ceq 'Kill' }, $true))
if ($errors.Count -ne 0 -or $kills.Count -ne 1 -or $kills[0].Arguments.Count -ne 1 -or $kills[0].Arguments[0].Extent.Text -cne '$false') { throw 'Portal cleanup expanded beyond its directly held process.' }
'PASS: fifteen bounded direct-process toy scenarios, exact shared isolation and measured worker address; no bus, service, mount or native signal adapter.'
