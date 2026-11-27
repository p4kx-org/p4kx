$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-database.ps1')

<#
.SYNOPSIS
Hashes the exact bounded MSI Binary stream without executing or extracting the helper.
#>
function Get-P4kxMsiEmbeddedHelperDigest {
    param([Parameter(Mandatory)][string]$Path)
    $installer = New-Object -ComObject WindowsInstaller.Installer
    $database = $null
    $view = $null
    $record = $null
    $hash = [Security.Cryptography.IncrementalHash]::CreateHash([Security.Cryptography.HashAlgorithmName]::SHA256)
    try {
        $database = Invoke-P4kxComMethod $installer 'OpenDatabase' @([IO.Path]::GetFullPath($Path),0)
        $view = Invoke-P4kxComMethod $database 'OpenView' @('SELECT `Data` FROM `Binary` WHERE `Name` = ''P4kxPathHelper''')
        [void](Invoke-P4kxComMethod $view 'Execute' @())
        $record = Invoke-P4kxComMethod $view 'Fetch' @()
        if ($null -eq $record) { throw 'Missing embedded MSI helper stream.' }
        $size = [long](Get-P4kxComProperty $record 'DataSize' @(1))
        if ($size -le 0 -or $size -gt 32MB) { throw 'Embedded MSI helper exceeds its stream bound.' }
        $encoding = [Text.Encoding]::GetEncoding(28591,[Text.EncoderFallback]::ExceptionFallback,[Text.DecoderFallback]::ExceptionFallback)
        $remaining = $size
        while ($remaining -gt 0) {
            $request = [int][Math]::Min(65536,$remaining)
            $chunk = [string](Invoke-P4kxComMethod $record 'ReadStream' @(1,$request,1))
            if ($chunk.Length -ne $request) { throw 'Truncated embedded MSI helper stream.' }
            $hash.AppendData($encoding.GetBytes($chunk))
            $remaining -= $request
        }
        $extra = Invoke-P4kxComMethod $record 'ReadStream' @(1,1,1)
        if ([string]$extra -cne '') { throw 'Embedded MSI helper stream changed length.' }
        return [pscustomobject]@{length=$size;sha256=[Convert]::ToHexStringLower($hash.GetHashAndReset())}
    }
    finally {
        $hash.Dispose()
        Close-P4kxComObject $record
        if ($null -ne $view) { [void](Invoke-P4kxComMethod $view 'Close' @()); Close-P4kxComObject $view }
        Close-P4kxComObject $database
        Close-P4kxComObject $installer
    }
}
