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

function Wait-P4kxPackageCompletion {
    param([Diagnostics.Process]$Process, [Diagnostics.Stopwatch]$Timer, [long]$Milliseconds,
        [Threading.Tasks.Task[]]$OutputTasks, [string]$ExitTimeout, [string]$OutputTimeout)
    $remaining = [int][Math]::Max(0L, $Milliseconds - $Timer.ElapsedMilliseconds)
    if (-not $Process.WaitForExit($remaining)) { throw $ExitTimeout }
    $remaining = [int][Math]::Max(0L, $Milliseconds - $Timer.ElapsedMilliseconds)
    if (-not [Threading.Tasks.Task]::WaitAll($OutputTasks, $remaining)) { throw $OutputTimeout }
}

function Close-P4kxPackageProcess {
    param([Diagnostics.Process]$Process, [bool]$Started, [string]$Name,
        [IO.StreamReader]$OutputReader, [IO.StreamReader]$ErrorReader)
    try {
        if ($Started -and -not $Process.HasExited) {
            try { $Process.Kill($true) }
            catch [InvalidOperationException], [ComponentModel.Win32Exception] {
                if (-not $Process.HasExited) { throw }
            }
            if (-not $Process.WaitForExit(5000)) {
                throw "$Name did not stop after its watchdog."
            }
        }
    }
    finally {
        if ($null -ne $OutputReader) { $OutputReader.Dispose() }
        if ($null -ne $ErrorReader) { $ErrorReader.Dispose() }
        $Process.Dispose()
    }
}

function Invoke-P4kxPackageCapture {
    param([Diagnostics.Process]$Process, [long]$Milliseconds, [string]$Log,
        [string]$Name, [string]$ExitTimeout, [string]$OutputTimeout)
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $started = $false
    $outputReader = $null
    $errorReader = $null
    try {
        $started = $Process.Start()
        if (-not $started) { throw "$Name did not start." }
        $outputReader = $Process.StandardOutput
        $errorReader = $Process.StandardError
        $stdout = $outputReader.ReadToEndAsync()
        $stderr = $errorReader.ReadToEndAsync()
        Wait-P4kxPackageCompletion $Process $timer $Milliseconds @($stdout, $stderr) $ExitTimeout $OutputTimeout
        $output = $stdout.GetAwaiter().GetResult()
        $errors = $stderr.GetAwaiter().GetResult()
        $output + $errors | Set-Content -LiteralPath $Log -Encoding utf8NoBOM
        return @{ stdout = $output; exit_code = $Process.ExitCode }
    }
    finally {
        Close-P4kxPackageProcess $Process $started $Name $outputReader $errorReader
    }
}

function Invoke-P4kxPackageProcess {
    param([string]$Executable, [string[]]$Arguments, [string]$Directory, [string]$Log,
        [ValidateRange(1,2147483)][int]$Seconds = 120)
    $process = New-P4kxPackageProcess -Executable $Executable -Arguments $Arguments -Directory $Directory
    $timeout = "Packaged application timed out: $Executable"
    $result = Invoke-P4kxPackageCapture $process ([long]$Seconds * 1000) $Log "Packaged application $Executable" $timeout "$timeout; stdout/stderr did not reach EOF"
    if ($result.exit_code -ne 0) { throw "Packaged application exited $($result.exit_code): $Executable; see $Log" }
    return $result.stdout
}

function Invoke-P4kxPackageGuiSmoke {
    param([string]$Executable, [string[]]$Arguments, [string]$Directory, [string]$Os)
    $process = New-P4kxPackageProcess $Executable $Arguments $Directory
    $process.StartInfo.Environment['P4KX_SMOKE_FRAMES'] = '3'
    if ($Os -ceq 'macos') { $process.StartInfo.Environment['WGPU_BACKEND'] = 'metal' }
    $timeout = 'Packaged GUI frame smoke timed out.'
    $pipeTimeout = 'Packaged GUI frame smoke timed out waiting for stdout/stderr EOF.'
    $result = Invoke-P4kxPackageCapture $process 120000 (Join-Path $Directory 'gui.log') 'Packaged GUI' $timeout $pipeTimeout
    if ($result.exit_code -ne 0) { throw "Packaged GUI exited $($result.exit_code)." }
}
