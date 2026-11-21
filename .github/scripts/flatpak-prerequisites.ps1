$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'portal-process.ps1')
. (Join-Path $PSScriptRoot 'flatpak-prerequisite-proof.ps1')

function Assert-P4kxFlatpakPrerequisiteOutcome {
    <#
    .SYNOPSIS
    Requires the measured unsupported-interface refusal without accepting transport failures.
    #>
    param($Result, $InterfaceVersion = 4)
    $expected = Get-P4kxFlatpakApi4Version $InterfaceVersion
    if (($Result.exit_code -isnot [int] -and $Result.exit_code -isnot [long]) -or
        $Result.exit_code -ne 1 -or $Result.stderr -isnot [string] -or
        -not $Result.stderr.Contains('[p4kx.runtime.documents-api5]', [StringComparison]::Ordinal)) {
        throw 'The packaged app did not report its typed Documents API5 prerequisite.'
    }
    $prefix = '[p4kx.documents.unsupported-interface:'
    $pattern = [regex]::Escape($prefix) + '(0|[1-9][0-9]*)\]'
    $codes = [regex]::Matches($Result.stderr, $pattern)
    if ($codes.Count -eq 0 -or $codes.Count -ne [regex]::Matches($Result.stderr, [regex]::Escape($prefix)).Count) {
        throw 'API4 negative requires a canonical typed unsupported-interface marker.'
    }
    foreach ($code in $codes) {
        $observed = [uint32]0
        if (-not [uint32]::TryParse($code.Groups[1].Value, [ref]$observed) -or $observed -ne $expected) {
            throw 'The app refused a different interface version than the measured API4 capability.'
        }
    }
    $header = '(?m)^[^\r\n]*\[p4kx\.runtime\.documents-api5\][^\r\n]*' + $pattern
    if (-not [regex]::IsMatch($Result.stderr, $header)) {
        throw 'The typed unsupported-interface cause is not in the operational prerequisite diagnostic.'
    }
    return @{refusal_reason='unsupported-interface'; interface_version=$expected}
}

function Test-P4kxFlatpakPrerequisites {
    param($State, $Platform, [string]$Fixture, [string]$Directory, $InterfaceVersion)
    $expected = Get-P4kxFlatpakApi4Version $InterfaceVersion
    New-Item -ItemType Directory -Path $Directory | Out-Null
    $failures = [Collections.Generic.List[Exception]]::new()
    $proof = @()
    $root = Join-Path $State.data_directory 'native-package-smoke'
    if (Test-Path -LiteralPath $root) { throw 'API4 negative profile baseline must not already exist.' }
    foreach ($name in @('p4kx', 'p4kx-mcp', 'p4kx-gui')) {
        try {
            $launcher = $State.launchers[$name]
            foreach ($argument in @('--help', '--version')) {
                $command = $launcher.prefix + @($argument)
                $result = Invoke-P4kxPortalProcess $launcher.executable $command $Directory "$name-$($argument.Substring(2))" -Milliseconds 30000
                if ($result.exit_code -ne 0 -or -not $result.stdout.Trim()) { throw 'Help or version failed before the runtime prerequisite.' }
            }
            $arguments = if ($name -ceq 'p4kx') { @('extract', (Join-Path $Fixture 'native-smoke.p4k'), '--filter', '*', '--out', (Join-Path $Directory 'forbidden-output')) } else { @() }
            $result = Invoke-P4kxPortalProcess $launcher.executable ($launcher.prefix + $arguments) $Directory "$name-prerequisite" -Milliseconds 30000
            $refusal = Assert-P4kxFlatpakPrerequisiteOutcome $result $expected
            $proof += @{application=$name; prerequisite='expected-refusal'; help='success'; version='success';
                refusal_reason=$refusal.refusal_reason; interface_version=$refusal.interface_version; exit_code=$result.exit_code}
        }
        catch { $failures.Add($_.Exception); $proof += @{application=$name; prerequisite='failure'} }
    }
    if ((Test-Path -LiteralPath $root) -or (Test-Path -LiteralPath (Join-Path $Directory 'forbidden-output'))) {
        $failures.Add([InvalidOperationException]::new('API4 refusal created an operational profile, key root or output.'))
    }
    @{schema=1; applications=$proof; runtime_pass=$false} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Directory 'prerequisites.json') -Encoding utf8NoBOM
    if ($failures.Count -ne 0) { throw [AggregateException]::new('Native API4 prerequisite negative checks failed.', $failures) }
    return @{schema=1; outcome='expected-refusal'; applications=$proof; runtime_pass=$false}
}
