function Get-P4kxLinuxWindowSmokeCases {
    'the_window_starts_draws_frames_and_exits'
    'the_wayland_window_draws_hidpi_frames_and_exits'
}

function Assert-P4kxLinuxWindowSmokeReport([xml]$Report) {
    $cases = @($Report.SelectNodes('/testsuites/testsuite/testcase'))
    foreach ($required in (Get-P4kxLinuxWindowSmokeCases)) {
        $matchingCases = @($cases | Where-Object { $_.GetAttribute('name') -ceq $required })
        if ($matchingCases.Count -ne 1 -or $null -ne $matchingCases[0].SelectSingleNode('failure|error|skipped')) {
            throw "Linux window smoke did not record exactly one pass for $required."
        }
    }
}
