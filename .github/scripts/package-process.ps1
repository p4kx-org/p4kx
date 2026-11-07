$ErrorActionPreference = 'Stop'

function New-P4kxPackageProcess {
    param([string]$Executable, [string[]]$Arguments, [string]$Directory)
    $start = [System.Diagnostics.ProcessStartInfo]::new($Executable)
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.RedirectStandardInput = $true
    $start.StandardOutputEncoding = [Text.UTF8Encoding]::new($false, $true)
    $start.StandardErrorEncoding = [Text.UTF8Encoding]::new($false, $true)
    $start.StandardInputEncoding = [Text.UTF8Encoding]::new($false, $true)
    foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
    foreach ($name in @('P4KX_ARCHIVE_KEY', 'P4KX_KEY_FILE', 'P4KX_CONFIG_DIR', 'P4KX_DATA_P4K', 'P4KX_FOLDERS_ROOT')) {
        [void]$start.Environment.Remove($name)
    }
    $start.Environment['P4KX_FOLDERS_ROOT'] = [IO.Path]::GetFullPath($Directory)
    $start.Environment['P4KX_GAME_CHECK'] = 'off'
    $start.Environment['P4KX_LAUNCHER_LOGS'] = Join-Path $Directory 'no-launcher-logs'
    $start.Environment['P4KX_LAUNCHER_DIR'] = Join-Path $Directory 'no-launcher'
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    return $process
}

function Invoke-P4kxPackageProcess {
    param([string]$Executable, [string[]]$Arguments, [string]$Directory, [string]$Log, [int]$Seconds = 120)
    $process = New-P4kxPackageProcess -Executable $Executable -Arguments $Arguments -Directory $Directory
    try {
        [void]$process.Start()
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit($Seconds * 1000)) {
            $process.Kill($true)
            $process.WaitForExit()
            throw "Packaged application timed out: $Executable"
        }
        $output = $stdout.GetAwaiter().GetResult()
        $errors = $stderr.GetAwaiter().GetResult()
        $output + $errors | Set-Content -LiteralPath $Log -Encoding utf8NoBOM
        if ($process.ExitCode -ne 0) { throw "Packaged application exited $($process.ExitCode): $Executable; see $Log" }
        return $output
    }
    finally { $process.Dispose() }
}
