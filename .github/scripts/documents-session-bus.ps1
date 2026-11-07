$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'portal-process.ps1')

function ConvertFrom-P4kxDocumentsBusAddress {
    <#
    .SYNOPSIS
    Reads one complete Unix bus address without accepting additional output or endpoints.
    #>
    param([string]$Text, [string]$ExpectedListener)
    if ($Text.Length -gt 4096) { throw 'Session bus address exceeds its bound.' }
    if (-not $Text.Contains("`n")) { return $null }
    $match = [regex]::Match($Text, '\A(unix:(?:path|abstract)=(?:[A-Za-z0-9_/.*-]|%[0-9A-Fa-f]{2})+(?:,guid=[0-9a-f]{32})?)\r?\n\z')
    if (-not $match.Success) { throw 'Session bus did not print exactly one bounded Unix address.' }
    $address = $match.Groups[1].Value
    if ($ExpectedListener -and $address.Split(',')[0] -cne $ExpectedListener) { throw 'Session bus listener differs from its owned configuration.' }
    return $address
}

function Get-P4kxDocumentsBusLaunch {
    param([string]$Config)
    $arguments = @('--nofork', '--print-address=1')
    $listener = ''
    if ($Config) {
        $document = [Xml.XmlDocument]::new(); $document.XmlResolver = $null
        $document.LoadXml((Get-Content -LiteralPath $Config -Raw))
        $nodes = @($document.SelectNodes('/busconfig/listen'))
        if ($nodes.Count -ne 1 -or $nodes[0].InnerText -cnotmatch '^unix:abstract=p4kx-docs-[0-9a-f]{32}$') {
            throw 'Private session has no single owned abstract listener.'
        }
        $listener = $nodes[0].InnerText
        $arguments += "--config-file=$Config"
    }
    else { $arguments += '--session' }
    return @{arguments=$arguments; listener=$listener}
}

function New-P4kxDocumentsProcess {
    param([string]$Executable, [string[]]$Arguments, [string]$Directory, [hashtable]$Environment, [string]$Address)
    $process = New-P4kxPackageProcess $Executable $Arguments $Directory
    foreach ($entry in $Environment.GetEnumerator()) { $process.StartInfo.Environment[$entry.Key] = [string]$entry.Value }
    foreach ($name in @('DBUS_STARTER_ADDRESS', 'DBUS_STARTER_BUS_TYPE', 'DBUS_SESSION_BUS_ADDRESS')) {
        [void]$process.StartInfo.Environment.Remove($name)
    }
    if ($Address) { $process.StartInfo.Environment['DBUS_SESSION_BUS_ADDRESS'] = $Address }
    return @{process=$process; started=$false; channels=@(); proof=@{started=$false; exited=$false; signal_contract='direct-process-only'}}
}

function Start-P4kxDocumentsProcess {
    param([hashtable]$Owned)
    if (-not $Owned.process.Start()) { throw 'Owned session process did not start.' }
    $Owned.started = $true
    $Owned.process.StandardInput.Close()
    $Owned.channels = New-P4kxPortalChannels $Owned.process
}

function Wait-P4kxDocumentsBusAddress {
    param([hashtable]$Bus, [string]$Listener, [int]$Milliseconds = 5000)
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while ($timer.ElapsedMilliseconds -lt $Milliseconds) {
        Update-P4kxPortalChannels $Bus.channels 131072
        if ($Bus.process.HasExited) { throw 'Owned session bus exited before worker startup.' }
        $address = ConvertFrom-P4kxDocumentsBusAddress $Bus.channels[0].text.ToString() $Listener
        if ($address) { return $address }
        Wait-P4kxPortalChannels $Bus.channels
    }
    throw 'Owned session bus address timed out.'
}