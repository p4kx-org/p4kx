$ErrorActionPreference = 'Stop'
$runtime = Join-Path $env:RUNNER_TEMP 'p4kx-wayland'
New-Item -ItemType Directory -Path $runtime | Out-Null
[IO.File]::SetUnixFileMode($runtime,
    ([IO.UnixFileMode]::UserRead -bor [IO.UnixFileMode]::UserWrite -bor [IO.UnixFileMode]::UserExecute))

$display = 'wayland-0'
$socket = Join-Path $runtime $display
$log = Join-Path $runtime 'weston.log'
$compositor = [Diagnostics.ProcessStartInfo]::new('weston')
$compositor.UseShellExecute = $false
$compositor.Environment['XDG_RUNTIME_DIR'] = $runtime
foreach ($name in @('DISPLAY', 'WAYLAND_DISPLAY', 'WAYLAND_SOCKET')) {
    [void]$compositor.Environment.Remove($name)
}
foreach ($argument in @('--backend=headless', '--renderer=pixman', '--shell=kiosk', '--no-config',
    '--idle-time=0', '--width=1280', '--height=800', '--scale=2', "--socket=$display", "--log=$log")) {
    $compositor.ArgumentList.Add($argument)
}

$weston = [Diagnostics.Process]::Start($compositor)
try {
    $deadline = [Diagnostics.Stopwatch]::StartNew()
    while (-not (Test-Path -LiteralPath $socket)) {
        if ($weston.HasExited -or $deadline.Elapsed.TotalSeconds -ge 15) {
            throw 'The isolated Weston compositor did not open its Wayland socket.'
        }
        Start-Sleep -Milliseconds 100
    }

    $env:WAYLAND_DISPLAY = $null
    $env:WAYLAND_SOCKET = $null
    $env:P4KX_TEST_WAYLAND_RUNTIME_DIR = $runtime
    $filter = 'test(the_window_starts_draws_frames_and_exits) | test(the_wayland_window_draws_hidpi_frames_and_exits)'
    $nextest = @('nextest', 'run', '--archive-file', 'nextest-archive.tar.zst', '--workspace-remap', '.',
        '--extract-to', '.', '--extract-overwrite', '--run-ignored', 'only', '--no-tests', 'fail', '-E', $filter)
    & xvfb-run --auto-servernum cargo @nextest
    if ($LASTEXITCODE -ne 0) { throw 'Native Linux X11 or HiDPI Wayland window smoke failed.' }
}
finally {
    try {
        if (-not $weston.HasExited) {
            $weston.Kill($true)
            $weston.WaitForExit()
        }
    }
    finally {
        $weston.Dispose()
        if (Test-Path -LiteralPath $log) { Get-Content -LiteralPath $log }
    }
}
