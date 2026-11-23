$ErrorActionPreference = 'Stop'

function Test-P4kxWindowsDesktopTarget($Platform) {
    return $Platform.os -ceq 'windows' -and $Platform.arch -ceq 'amd64' -and
        $Platform.target -ceq 'x86_64-pc-windows-msvc'
}

function Assert-P4kxWindowsDesktopReceiptRows($Rows, $Contract) {
    if ($Rows -isnot [array]) { throw 'Windows AMD64 desktop evidence must be a JSON array.' }
    $targets = @($Contract.targets | Where-Object { Test-P4kxWindowsDesktopTarget $_ } | Select-Object -ExpandProperty target)
    foreach ($record in $Rows) {
        if ($record -isnot [PSCustomObject] -or $record.target -isnot [string] -or $record.target -cnotin $targets) {
            throw 'Windows AMD64 desktop evidence identifies an unsupported device target.'
        }
    }
}

function Get-P4kxWindowsDesktopRequiredChecks {
    return @('file_dialogs', 'clipboard', 'viewer', 'audio_start', 'audio_pause', 'audio_seek', 'audio_stop', 'no_audio_device')
}

function Assert-P4kxWindowsDesktopEvidence {
    param(
        [Parameter(Mandatory)]$Runtime,
        [Parameter(Mandatory)]$Platform,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)][string]$Revision,
        [Parameter(Mandatory)][string]$ReleaseRunId,
        [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Records
    )

    if (-not (Test-P4kxWindowsDesktopTarget $Platform)) { throw 'Expected a native Windows AMD64 target.' }
    $workflow = "https://github.com/p4kx-org/p4kx/actions/runs/$ReleaseRunId"
    if ($ReleaseRunId -cnotmatch '\A[1-9][0-9]*\z' -or $Runtime.workflow_run -cne $workflow -or
        $Runtime.workflow_attempt -isnot [string] -or $Runtime.workflow_attempt -cnotmatch '\A[1-9][0-9]*\z' -or
        $Runtime.scope -cne 'native-synthetic' -or $Runtime.target -cne $Platform.target -or
        $Runtime.version -cne $Version -or $Runtime.revision -cne $Revision -or
        $Revision -cnotmatch '\A(?:[0-9a-f]{40}|[0-9a-f]{64})\z') {
        throw 'The verified package receipt does not identify the required release revision, target, run and attempt.'
    }
    if ($Records.Count -eq 0) { throw "Missing Windows AMD64 desktop evidence for $($Platform.target)." }
    $required = @(Get-P4kxWindowsDesktopRequiredChecks)
    $passed = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($record in $Records) {
        Assert-P4kxWindowsDesktopRecord $record $Runtime $Platform
        Assert-P4kxWindowsDesktopHost $record $Platform
        Assert-P4kxWindowsDesktopChecks $record $required $passed
    }
    foreach ($name in $required) {
        if (-not $passed.Contains($name)) { throw "Required Windows AMD64 desktop check '$name' has no successful record for $($Platform.target)." }
    }
}

function Assert-P4kxWindowsDesktopRecord($Record, $Runtime, $Platform) {
    if ($Record -isnot [PSCustomObject] -or ($Record.schema -isnot [int] -and $Record.schema -isnot [long]) -or
        $Record.schema -ne 1 -or $Record.scope -isnot [string] -or $Record.scope -cne 'native-device') {
        throw 'Invalid Windows AMD64 desktop receipt schema or scope.'
    }
    foreach ($field in @('target', 'version', 'revision', 'workflow_run', 'workflow_attempt')) {
        if ($Record.$field -isnot [string] -or $Record.$field -cne $Runtime.$field) {
            throw "Windows AMD64 desktop receipt $field differs from the verified candidate."
        }
    }
    if ($Record.fixture_revision -isnot [string] -or $Record.fixture_revision -cne $Runtime.revision) {
        throw 'Windows AMD64 desktop fixture revision differs from the candidate.'
    }
    if ($Record.package -isnot [string] -or $Record.package -cnotin $Platform.packages) {
        throw 'Windows AMD64 desktop package is outside the candidate target.'
    }
    $filename = Get-P4kxAssetName -Target $Platform.target -Version $Runtime.version -Package $Record.package
    $packages = @($Runtime.package_smokes | Where-Object package -CEQ $Record.package)
    if ($packages.Count -ne 1) { throw 'The tested package is missing or duplicated in verified package evidence.' }
    $package = $packages[0]
    if ($package.target -cne $Platform.target -or $Record.package_filename -isnot [string] -or
        $Record.package_filename -cne $filename -or $Record.archive_sha256 -isnot [string] -or
        $Record.archive_sha256 -cnotmatch '\A[0-9a-f]{64}\z' -or $Record.archive_sha256 -cne $package.archive_sha256) {
        throw 'Windows AMD64 desktop package filename, target or SHA-256 differs from the verified candidate.'
    }
    $gui = @($package.binaries | Where-Object name -CEQ 'p4kx-gui')
    if ($gui.Count -ne 1 -or $gui[0].machine -ne (Get-P4kxNativeMachine -Platform $Platform) -or
        $Record.gui_sha256 -isnot [string] -or $Record.gui_sha256 -cnotmatch '\A[0-9a-f]{64}\z' -or
        $Record.gui_sha256 -cne $gui[0].sha256) {
        throw 'Windows AMD64 desktop GUI machine or SHA-256 differs from the tested package.'
    }
}

function Assert-P4kxWindowsDesktopHost($Record, $Platform) {
    if ($Record.host -isnot [PSCustomObject] -or $Record.host.os -isnot [string] -or $Record.host.os -cne $Platform.os -or
        $Record.host.architecture -isnot [string] -or $Record.host.architecture -cne $Platform.arch) {
        throw 'Windows AMD64 desktop host OS or native CPU differs from the package target.'
    }
    foreach ($field in @('os_version', 'os_build_or_kernel', 'desktop_session', 'gpu', 'gpu_driver', 'audio_backend')) {
        if ($Record.host.$field -isnot [string] -or [string]::IsNullOrWhiteSpace($Record.host.$field)) {
            throw "Windows AMD64 desktop host field '$field' is missing."
        }
    }
    $date = [DateOnly]::MinValue
    if ($Record.tested_on -isnot [string] -or -not [DateOnly]::TryParseExact(
        $Record.tested_on, 'yyyy-MM-dd', [Globalization.CultureInfo]::InvariantCulture,
        [Globalization.DateTimeStyles]::None, [ref]$date
    )) {
        throw 'Windows AMD64 desktop evidence needs the observed test date (yyyy-MM-dd).'
    }
}

function Assert-P4kxWindowsDesktopChecks($Record, [string[]]$Required, [Collections.Generic.HashSet[string]]$Passed) {
    $checks = $Record.checks
    if ($checks -isnot [PSCustomObject]) { throw 'Windows AMD64 desktop checks must be a JSON object.' }
    $names = @($checks.PSObject.Properties.Name)
    foreach ($name in ($Required + 'hardware_gpu')) {
        $check = $checks.$name
        if ($name -cnotin $names -or $check -isnot [PSCustomObject] -or $check.outcome -isnot [string] -or
            $check.outcome -cnotin @('success', 'failed', 'unavailable', 'not_checked') -or
            $check.observation -isnot [string] -or [string]::IsNullOrWhiteSpace($check.observation)) {
            throw "Windows AMD64 desktop result '$name' needs its exact check name, outcome and observed evidence."
        }
        if ($check.outcome -cne 'success') { continue }
        if ($name -ceq 'file_dialogs') {
            if ($check.expected_sha256 -isnot [string] -or $check.expected_sha256 -cnotmatch '\A[0-9a-f]{64}\z' -or
                $check.observed_sha256 -isnot [string] -or $check.observed_sha256 -cne $check.expected_sha256) {
                throw 'Successful Windows AMD64 file_dialogs must retain matching expected and extracted SHA-256 values.'
            }
        }
        if ($name -ceq 'no_audio_device' -and ($check.test -isnot [string] -or
            $check.test -cne 'a_missing_device_is_reported_to_the_player_and_does_not_panic')) {
            throw 'Successful Windows AMD64 no_audio_device evidence names the required native test.'
        }
        [void]$Passed.Add($name)
    }
}
