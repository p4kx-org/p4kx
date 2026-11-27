$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-installation.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')
. (Join-Path $PSScriptRoot 'portal-measurement.ps1')
. (Join-Path $PSScriptRoot 'documents-session-controller.ps1')

function ConvertTo-P4kxDocumentsSessionConfig {
    <#
    .SYNOPSIS
    Creates an owned session bus with first-priority official service activation.
    #>
    param([string]$ListenerName, [string]$ServiceDirectory)
    if ($ListenerName -cnotmatch '^p4kx-docs-[0-9a-f]{32}$') { throw 'Private session listener identity is invalid.' }
    foreach ($path in @($ServiceDirectory)) {
        if (-not $path.StartsWith('/', [StringComparison]::Ordinal) -or $path -match '[\r\n\0]' -or '..' -cin ($path -split '/')) {
            throw 'Session activation and socket paths must be absolute owned Linux paths.'
        }
    }
    $document = [Xml.XmlDocument]::new()
    $document.XmlResolver = $null
    $root = $document.CreateElement('busconfig')
    [void]$document.AppendChild($root)
    foreach ($entry in @(@('type', 'session'), @('listen', "unix:abstract=$ListenerName"), @('auth', 'EXTERNAL'), @('servicedir', $ServiceDirectory))) {
        $node = $document.CreateElement($entry[0]); $node.InnerText = $entry[1]; [void]$root.AppendChild($node)
    }
    [void]$root.AppendChild($document.CreateElement('standard_session_servicedirs'))
    $policy = $document.CreateElement('policy'); $policy.SetAttribute('context', 'default')
    foreach ($attributes in @(@{send_destination='*'; eavesdrop='true'}, @{eavesdrop='true'}, @{own='*'})) {
        $allow = $document.CreateElement('allow')
        foreach ($attribute in $attributes.GetEnumerator()) { $allow.SetAttribute($attribute.Key, $attribute.Value) }
        [void]$policy.AppendChild($allow)
    }
    [void]$root.AppendChild($policy)
    return $document.OuterXml
}

function ConvertTo-P4kxDocumentsActivation {
    param([ValidateSet('xdg-document-portal', 'xdg-permission-store')][string]$Name, [string]$Executable)
    if (-not $Executable.StartsWith('/', [StringComparison]::Ordinal) -or $Executable -match '[\r\n\0]') { throw 'Service activation requires one absolute executable path.' }
    $service = if ($Name -ceq 'xdg-document-portal') { 'org.freedesktop.portal.Documents' } else { 'org.freedesktop.impl.portal.PermissionStore' }
    $quoted = '"' + $Executable.Replace('\', '\\').Replace('"', '\"') + '"'
    return "[D-BUS Service]`nName=$service`nExec=$quoted`n"
}

function New-P4kxDocumentsSessionConfig {
    param([string]$Directory, $Build)
    Assert-P4kxDisposableLinuxRunner (Get-P4kxCurrentPlatform)
    if (Test-Path -LiteralPath $Directory) { throw 'Portal session configuration requires a fresh directory.' }
    $services = Join-Path $Directory 'services'
    New-Item -ItemType Directory -Path $services -Force | Out-Null
    $names = @{ 'xdg-document-portal'='org.freedesktop.portal.Documents'; 'xdg-permission-store'='org.freedesktop.impl.portal.PermissionStore' }
    foreach ($name in $names.Keys) {
        $found = @($Build.binaries | Where-Object name -CEQ $name)
        if ($found.Count -ne 1 -or (Get-FileHash -LiteralPath $found[0].path -Algorithm SHA256).Hash.ToLowerInvariant() -cne $found[0].sha256) { throw 'Official service executable source binding differs.' }
        if ((Get-ImageMachine -Path $found[0].path -Os linux) -ne (Get-P4kxLinuxPlatform $Build.target).elf_machine) { throw 'Activated Documents service is not a native target executable.' }
        ConvertTo-P4kxDocumentsActivation $name $found[0].path | Set-Content -LiteralPath (Join-Path $services "$($names[$name]).service") -Encoding utf8NoBOM
    }
    $config = Join-Path $Directory 'session.conf'
    $listener = 'p4kx-docs-' + [guid]::NewGuid().ToString('N')
    ConvertTo-P4kxDocumentsSessionConfig $listener $services | Set-Content -LiteralPath $config -Encoding utf8NoBOM
    return $config
}

function Get-P4kxDocumentsSessionEnvironment {
    param([string]$Directory, [string]$FlatpakDirectory)
    $variables = @{XDG_DATA_HOME='data'; XDG_CONFIG_HOME='config'; XDG_CACHE_HOME='cache'}
    $result = @{FLATPAK_USER_DIR=$FlatpakDirectory}
    foreach ($entry in $variables.GetEnumerator()) {
        $path = Join-Path $Directory $entry.Value
        New-Item -ItemType Directory -Path $path -Force | Out-Null
        $result[$entry.Key] = $path
    }
    return $result
}

function Invoke-P4kxDocumentsSession {
    param([string]$Script, [string[]]$Arguments, [string]$Directory, [string]$Name, [hashtable]$Environment, [string]$Config)
    $deadline = if ($Name -ceq 'cleanup-only') { 300000 } else { 1800000 }
    return Invoke-P4kxOwnedDocumentsProcesses $Script $Arguments $Directory $Name $Environment $Config -Milliseconds $deadline
}
function Assert-P4kxOwnedDocumentsSession {
    param([ValidateSet('stock', 'api5', 'cleanup-only')][string]$Role)
    if ($env:P4KX_PORTAL_SESSION_ROLE -cne $Role -or $env:DBUS_SESSION_BUS_ADDRESS -cnotmatch '^unix:' -or
        $env:DBUS_SESSION_BUS_ADDRESS -ceq $env:P4KX_PORTAL_PARENT_BUS) { throw 'Native portal checks require the independently owned session bus.' }
}
