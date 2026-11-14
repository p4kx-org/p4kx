$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')
. (Join-Path $PSScriptRoot 'portal-measurement.ps1')
. (Join-Path $PSScriptRoot 'documents-session-proof.ps1')

function Get-P4kxPortalUserId {
    $value = (Invoke-P4kxLinuxTool id @('-u') | Out-String).Trim()
    if ($value -cnotmatch '^[0-9]+$') { throw 'Native portal user ID is not a decimal scalar.' }
    return [uint32]::Parse($value, [Globalization.CultureInfo]::InvariantCulture)
}

function Get-P4kxPortalProcessStart {
    param([int]$ProcessId)
    $file = "/proc/$ProcessId/stat"
    try { $stat = [IO.File]::ReadAllText($file) }
    catch [IO.FileNotFoundException] { return $null }
    catch [IO.DirectoryNotFoundException] { return $null }
    if ($stat -cnotmatch '^[0-9]+ \(.*\) ([^\r\n]+)$') { throw 'Portal process identity has an invalid proc record.' }
    $fields = $Matches[1] -split ' '
    if ($fields.Count -lt 20 -or $fields[19] -cnotmatch '^[0-9]+$') { throw 'Portal process start identity is missing.' }
    return $fields[19]
}

function Get-P4kxPortalNameOwner {
    param([string]$Service, [string]$Directory, [string]$Name)
    $value = Invoke-P4kxPortalMeasurement busctl @('--user', '--timeout=2', 'call', 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetNameOwner', 's', $Service) $Directory $Name
    if ($value -cnotmatch '^s "(:[0-9]+\.[0-9]+)"\s*$') { throw 'Portal service has no unambiguous unique bus owner.' }
    return $Matches[1]
}

function Get-P4kxPortalServiceIdentity {
    param([string]$Service, [string]$Directory, [string]$Name, [string]$ExpectedExecutable)
    $owner = Get-P4kxPortalNameOwner $Service $Directory "$Name-owner-before"
    $value = Invoke-P4kxPortalMeasurement busctl @('--user', '--timeout=2', 'call', 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixProcessID', 's', $owner) $Directory "$Name-pid"
    if ($value -cnotmatch '^u ([1-9][0-9]*)\s*$') { throw 'Portal owner has no positive Unix process identity.' }
    $processId = [int]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
    $credentials = Invoke-P4kxPortalMeasurement busctl @('--user', '--timeout=2', 'call', 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'GetConnectionUnixUser', 's', $owner) $Directory "$Name-uid"
    if ($credentials -cnotmatch '^u ([0-9]+)\s*$') { throw 'Portal owner has no Unix user credential.' }
    $userId = [uint32]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
    if ($userId -ne (Get-P4kxPortalUserId)) { throw 'Portal owner belongs to a different Unix user.' }
    $start = Get-P4kxPortalProcessStart $processId
    $executable = [IO.FileInfo]::new("/proc/$processId/exe").ResolveLinkTarget($true).FullName
    if ($executable -cne $ExpectedExecutable -or -not $start) { throw 'Portal owner is not the expected official executable.' }
    $hash = (Get-FileHash -LiteralPath "/proc/$processId/exe" -Algorithm SHA256).Hash.ToLowerInvariant()
    if ((Get-P4kxPortalNameOwner $Service $Directory "$Name-owner-after") -cne $owner -or
        (Get-P4kxPortalProcessStart $processId) -cne $start) { throw 'Portal owner changed during source identity measurement.' }
    return @{service=$Service; owner=$owner; pid=$processId; start=$start; uid=$userId; executable=$executable; sha256=$hash}
}

function Save-P4kxDocumentsSessionIdentity {
    param([string]$Directory, $Build, [ValidateSet('initial', 'final')][string]$Stage = 'final')
    $names = @{'xdg-document-portal'='org.freedesktop.portal.Documents'; 'xdg-permission-store'='org.freedesktop.impl.portal.PermissionStore'}
    $services = @(foreach ($name in $names.Keys) {
        $present = Invoke-P4kxPortalMeasurement busctl @('--user', '--timeout=2', 'call', 'org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus', 'NameHasOwner', 's', $names[$name]) $Directory "$name-presence"
        if ($present -cmatch '^b false\s*$') { continue }
        if ($present -cnotmatch '^b true\s*$') { throw 'Portal ownership inventory has an invalid boolean reply.' }
        if ($Build) { $path = @($Build.binaries | Where-Object name -CEQ $name)[0].path }
        else {
            $paths = @(Invoke-P4kxLinuxTool dpkg-query @('--listfiles', 'xdg-desktop-portal') | Where-Object { $_.EndsWith("/$name", [StringComparison]::Ordinal) -and (Test-Path -LiteralPath $_ -PathType Leaf) })
            if ($paths.Count -ne 1) { throw 'Stock portal executable package identity is ambiguous.' }
            $path = $paths[0]
        }
        $identity = Get-P4kxPortalServiceIdentity $names[$name] $Directory $name $path
        if ($Build -and $identity.sha256 -cne @($Build.binaries | Where-Object name -CEQ $name)[0].sha256) { throw 'Running portal differs from the native service build.' }
        $identity
    })
    $observedServices = @($services)
    $initial = Join-Path $Directory 'session-services-initial.json'
    if ($Stage -ceq 'final' -and (Test-Path -LiteralPath $initial)) {
        $previous = Get-Content -LiteralPath $initial -Raw | ConvertFrom-Json
        if ($previous.session -cne $env:DBUS_SESSION_BUS_ADDRESS) { throw 'Initial portal inventory belongs to another session.' }
        foreach ($record in $previous.services) {
            if (@($services | Where-Object { $_.pid -eq $record.pid -and $_.start -ceq $record.start }).Count -eq 0) { $services += $record }
        }
    }
    $proof = @{session=$env:DBUS_SESSION_BUS_ADDRESS; user_uid=(Get-P4kxPortalUserId); stage=$Stage; inventory_complete=$true; services=$services; observed_services=$observedServices}
    $encoded = $proof | ConvertTo-Json -Depth 6
    $encoded | Set-Content -LiteralPath (Join-Path $Directory "session-services-$Stage.json") -Encoding utf8NoBOM
    $encoded | Set-Content -LiteralPath (Join-Path $Directory 'session-services.json') -Encoding utf8NoBOM
}

function Assert-P4kxDocumentsSessionEnded {
    param([string]$Directory, [string]$MountPoint)
    $proof = Get-Content -LiteralPath (Join-Path $Directory 'session-services.json') -Raw | ConvertFrom-Json
    if ($proof.inventory_complete -ne $true -or $proof.stage -cne 'final') { throw 'Final portal service ownership inventory is incomplete; another session is forbidden.' }
    $controller = Get-Content -LiteralPath (Join-Path $Directory 'session-controller.json') -Raw | ConvertFrom-Json
    Assert-P4kxDocumentsControllerEvidence $controller $proof.session ([IO.Path]::GetFileName($Directory))
    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        $alive = @($proof.services | Where-Object { (Get-P4kxPortalProcessStart $_.pid) -ceq $_.start })
        $mount = Invoke-P4kxPortalProcess findmnt @('--kernel', '--json', '--mountpoint', $MountPoint) $Directory 'teardown-mount'
        if ($mount.exit_code -eq 1 -and -not $mount.stdout.Trim() -and -not $mount.stderr.Trim() -and $alive.Count -eq 0) {
            @{source_session=$proof.session; processes_exited='success'; mount_gone='success'; controller_processes_exited='success'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Directory 'teardown.json') -Encoding utf8NoBOM
            return
        }
        if ($mount.exit_code -cnotin @(0, 1)) { throw 'Portal teardown mount probe failed.' }
        Start-Sleep -Milliseconds 100
    } while ($timer.ElapsedMilliseconds -lt 10000)
    throw 'Owned portal processes or FUSE mount survived their session; next session is forbidden.'
}
