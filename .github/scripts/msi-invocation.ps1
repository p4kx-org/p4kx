$ErrorActionPreference = 'Stop'

<#
.SYNOPSIS
Resolves an existing MSI file from the PowerShell location to its native absolute path.
#>
function Resolve-P4kxMsiInput {
    param([Parameter(Mandatory)][string]$Path)
    $resolved = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Path)
    $file = Get-Item -LiteralPath $resolved
    if ($file.PSIsContainer -or $file.Extension -ine '.msi') { throw 'Installer input must be an existing MSI file.' }
    return $file.FullName
}

<#
.SYNOPSIS
Builds a hidden, quiet installer invocation while preserving each argument boundary.
#>
function New-P4kxMsiexecStartInfo {
    param([string[]]$Arguments, [string]$Log)
    $start = [Diagnostics.ProcessStartInfo]::new('msiexec.exe')
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    foreach ($argument in $Arguments + @('/qn', '/norestart', '/l*v', $Log)) { $start.ArgumentList.Add($argument) }
    return $start
}
