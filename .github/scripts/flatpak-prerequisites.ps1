$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'portal-process.ps1')

function Assert-P4kxFlatpakPrerequisiteOutcome {
    <#
    .SYNOPSIS
    Requires the typed prerequisite refusal without classifying it as runtime success.
    #>
    param($Result)
    if ($Result.exit_code -ne 1 -or -not $Result.stderr.Contains('[p4kx.runtime.documents-api5]', [StringComparison]::Ordinal)) {
        throw 'The packaged app did not report its typed Documents API5 prerequisite.'
    }
}

function Test-P4kxFlatpakPrerequisites {
    param($State, $Platform, [string]$Fixture, [string]$Directory)
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
            $arguments = if ($name -ceq 'p4kx') { @('extract', (Join-Path $Fixture 'native-smoke.p4k'), '--out', (Join-Path $Directory 'forbidden-output')) } else { @() }
            $result = Invoke-P4kxPortalProcess $launcher.executable ($launcher.prefix + $arguments) $Directory "$name-prerequisite" -Milliseconds 30000
            Assert-P4kxFlatpakPrerequisiteOutcome $result
            $proof += @{application=$name; prerequisite='expected-refusal'; help='success'; version='success'}
        }
        catch { $failures.Add($_.Exception); $proof += @{application=$name; prerequisite='failure'} }
    }
    if ((Test-Path -LiteralPath $root) -or (Test-Path -LiteralPath (Join-Path $Directory 'forbidden-output'))) {
        $failures.Add([InvalidOperationException]::new('API4 refusal created an operational profile, key root or output.'))
    }
    @{applications=$proof; runtime_pass=$false} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Directory 'prerequisites.json') -Encoding utf8NoBOM
    if ($failures.Count -ne 0) { throw [AggregateException]::new('Native API4 prerequisite negative checks failed.', $failures) }
    return @{outcome='expected-refusal'; applications=$proof; runtime_pass=$false}
}
