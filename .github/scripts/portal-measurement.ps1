$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'portal-process.ps1')

function Invoke-P4kxPortalMeasurement {
    <#
    .SYNOPSIS
    Captures one portal probe with bounded output and an independent process deadline.
    #>
    param([string]$Executable, [string[]]$Arguments, [string]$Directory, [string]$Name,
        [int]$Milliseconds = 5000, [int]$MaximumBytes = 131072)
    $result = Invoke-P4kxPortalProcess $Executable $Arguments $Directory $Name -Milliseconds $Milliseconds -MaximumBytes $MaximumBytes -Environment @{}
    if ($result.exit_code -ne 0) { throw "Portal measurement exited $($result.exit_code); see its bounded artifact logs." }
    return $result.stdout
}

function ConvertFrom-P4kxDocumentsInterface {
    <#
    .SYNOPSIS
    Parses the observed Documents version and exact GetHostPaths signature.
    #>
    param([string]$Version, [string]$Introspection)
    if ($Version.Length -gt 64 -or [Text.Encoding]::UTF8.GetByteCount($Introspection) -gt 131072) { throw 'Documents interface evidence exceeds its limit.' }
    if ($Version -cnotmatch '^u ([0-9]+)\s*$') { throw 'Documents version reply is not one uint32 property.' }
    $number = [uint32]::Parse($Matches[1], [Globalization.CultureInfo]::InvariantCulture)
    $settings = [Xml.XmlReaderSettings]::new()
    $settings.DtdProcessing = [Xml.DtdProcessing]::Ignore
    $settings.XmlResolver = $null
    $settings.MaxCharactersInDocument = 131072
    $text = [IO.StringReader]::new($Introspection)
    $reader = [Xml.XmlReader]::Create($text, $settings)
    try {
        $document = [Xml.XmlDocument]::new()
        $document.XmlResolver = $null
        $document.Load($reader)
        $interfaces = @($document.SelectNodes('/node/interface[@name="org.freedesktop.portal.Documents"]'))
        if ($interfaces.Count -ne 1) { throw 'Documents introspection has no unambiguous interface.' }
        $methods = @($interfaces[0].SelectNodes('method[@name="GetHostPaths"]'))
        if ($methods.Count -gt 1) { throw 'Documents introspection has duplicate GetHostPaths methods.' }
        $available = $methods.Count -eq 1
        if ($available) {
            $arguments = @($methods[0].SelectNodes('arg'))
            $inputs = @($methods[0].SelectNodes('arg[@direction="in"]'))
            $outputs = @($methods[0].SelectNodes('arg[@direction="out"]'))
            if ($arguments.Count -ne 2 -or $inputs.Count -ne 1 -or $outputs.Count -ne 1 -or $inputs[0].type -cne 'as' -or $outputs[0].type -cne 'a{say}') {
                throw 'Documents GetHostPaths introspection has an unexpected signature.'
            }
        }
        return @{interface_version=$number; get_host_paths=$available}
    }
    finally { $reader.Dispose(); $text.Dispose() }
}
