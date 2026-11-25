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
    param([string]$Executable, [string[]]$Arguments, [string]$Directory, [string]$Log,
        [ValidateRange(1,2147483)][int]$Seconds = 120)
    $process = New-P4kxPackageProcess -Executable $Executable -Arguments $Arguments -Directory $Directory
    $budget = [long]$Seconds * 1000
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $started = $false
    $outputReader = $null
    $errorReader = $null
    try {
        $started = $process.Start()
        if (-not $started) { throw "Packaged application did not start: $Executable" }
        $outputReader = $process.StandardOutput
        $errorReader = $process.StandardError
        $stdout = $outputReader.ReadToEndAsync()
        $stderr = $errorReader.ReadToEndAsync()
        $remaining = [int][Math]::Max(0L, $budget - $timer.ElapsedMilliseconds)
        if (-not $process.WaitForExit($remaining)) {
            throw "Packaged application timed out: $Executable"
        }
        $remaining = [int][Math]::Max(0L, $budget - $timer.ElapsedMilliseconds)
        if (-not [Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($stdout, $stderr), $remaining)) {
            throw "Packaged application timed out: $Executable; stdout/stderr did not reach EOF"
        }
        $output = $stdout.GetAwaiter().GetResult()
        $errors = $stderr.GetAwaiter().GetResult()
        $output + $errors | Set-Content -LiteralPath $Log -Encoding utf8NoBOM
        if ($process.ExitCode -ne 0) { throw "Packaged application exited $($process.ExitCode): $Executable; see $Log" }
        return $output
    }
    finally {
        try {
            if ($started -and -not $process.HasExited) {
                try { $process.Kill($true) }
                catch [InvalidOperationException], [ComponentModel.Win32Exception] {
                    if (-not $process.HasExited) { throw }
                }
                if (-not $process.WaitForExit(5000)) {
                    throw "Packaged application did not stop after its watchdog: $Executable"
                }
            }
        }
        finally {
            if ($null -ne $outputReader) { $outputReader.Dispose() }
            if ($null -ne $errorReader) { $errorReader.Dispose() }
            $process.Dispose()
        }
    }
}
