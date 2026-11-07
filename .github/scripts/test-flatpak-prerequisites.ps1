<#
.SYNOPSIS
Checks packaged prerequisite arguments and refusal evidence with mocked process calls.
#>
param([Parameter(Mandatory)][string]$Directory)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'flatpak-prerequisites.ps1')
$Root = [IO.Path]::GetFullPath($Directory)
if (Test-Path -LiteralPath $Root) { throw 'Fixture root must be fresh.' }
New-Item -ItemType Directory -Path $Root | Out-Null
$script:Calls = [Collections.Generic.List[object]]::new()
$script:Fixture = Join-Path $Root 'synthetic-fixture'
$script:Output = Join-Path $Root 'negative-check'
$script:Prefixes = @{}
$script:TransportFailure = $false
$state = @{data_directory=(Join-Path $Root 'data'); launchers=@{}}
foreach ($name in @('p4kx', 'p4kx-mcp', 'p4kx-gui')) {
    $prefix = @('run', '--user', "--command=$name", 'org.p4kx.P4KX')
    $state.launchers[$name] = @{executable='mock-flatpak'; prefix=$prefix}
    $script:Prefixes[$name] = $prefix
}
function Assert-ExactArguments {
    param([string[]]$Actual, [string[]]$Expected)
    if ($Actual.Count -ne $Expected.Count) { return $false }
    for ($index = 0; $index -lt $Expected.Count; $index++) {
        if ($Actual[$index] -cne $Expected[$index]) { return $false }
    }
    return $true
}
function Invoke-P4kxPortalProcess {
    param([string]$Executable, [string[]]$Arguments, [string]$Directory, [string]$Name,
        [int]$Milliseconds = 5000, [int]$MaximumBytes = 131072, [hashtable]$Environment = @{})
    if ($Executable -cne 'mock-flatpak' -or $Directory -cne $script:Output -or $Milliseconds -ne 30000) {
        throw 'Unexpected process executable, task directory or bound.'
    }
    $script:Calls.Add(@{name=$Name; arguments=$Arguments})
    foreach ($application in @('p4kx', 'p4kx-mcp', 'p4kx-gui')) {
        foreach ($early in @('help', 'version')) {
            if ($Name -ceq "$application-$early") {
                if (-not (Assert-ExactArguments $Arguments ($script:Prefixes[$application] + @("--$early")))) {
                    throw 'Help/version arguments changed.'
                }
                return @{exit_code=0; stdout='synthetic help/version'; stderr=''}
            }
        }
        if ($Name -ceq "$application-prerequisite") {
            $operation = if ($application -ceq 'p4kx') {
                @('extract', (Join-Path $script:Fixture 'native-smoke.p4k'), '--filter', '*', '--out', (Join-Path $script:Output 'forbidden-output'))
            } else { @() }
            if (-not (Assert-ExactArguments $Arguments ($script:Prefixes[$application] + $operation))) {
                return @{exit_code=2; stdout=''; stderr='error: required <ENTRIES> were not provided'}
            }
            if ($script:TransportFailure -and $application -cne 'p4kx') {
                return @{exit_code=1; stdout=''; stderr='[p4kx.runtime.documents-api5] session bus unavailable (os error -1001)'}
            }
            return @{exit_code=1; stdout=''; stderr='[p4kx.runtime.documents-api5] [p4kx.documents.unsupported-interface:4] synthetic cause'}
        }
    }
    throw 'Unexpected mocked process name.'
}
$result = Test-P4kxFlatpakPrerequisites $state @{} $script:Fixture $script:Output 4
if ($script:Calls.Count -ne 9) { throw 'All three operational and six early calls are required.' }
if ($result.runtime_pass -or $result.outcome -cne 'expected-refusal' -or $result.applications.Count -ne 3) { throw 'Negative result must remain a refusal proof.' }
$proof = Get-Content -LiteralPath (Join-Path $script:Output 'prerequisites.json') -Raw | ConvertFrom-Json
if ($proof.runtime_pass -or @($proof.applications | Where-Object prerequisite -CNE 'expected-refusal').Count -ne 0) { throw 'Persisted proof must retain all three negative outcomes.' }
if ($proof.schema -ne 1 -or @($proof.applications | Where-Object {
    $_.refusal_reason -cne 'unsupported-interface' -or $_.interface_version -ne 4 -or $_.exit_code -ne 1
}).Count -ne 0) { throw 'Persisted negative proof lost the typed reason, version or actual exit code.' }
if ((Test-Path -LiteralPath (Join-Path $state.data_directory 'native-package-smoke')) -or
    (Test-Path -LiteralPath (Join-Path $script:Output 'forbidden-output'))) { throw 'Negative fixture created operational output.' }
foreach ($bad in @(@{exit_code=0; stderr='[p4kx.runtime.documents-api5]'}, @{exit_code=1; stderr='clap failure'}, @{exit_code=2; stderr='[p4kx.runtime.documents-api5]'})) {
    $rejected = $false
    try { Assert-P4kxFlatpakPrerequisiteOutcome $bad } catch { $rejected = $true }
    if (-not $rejected) { throw 'A malformed negative result was accepted.' }
}
$script:TransportFailure = $true
$script:Output = Join-Path $Root 'negative-transport'
$transportRejected = $false
try { Test-P4kxFlatpakPrerequisites $state @{} $script:Fixture $script:Output 4 | Out-Null }
catch [AggregateException] { $transportRejected = $_.Exception.InnerExceptions.Count -eq 2 }
if (-not $transportRejected -or $script:Calls.Count -ne 18) {
    throw 'Two transport failures were accepted, or valid CLI/remaining app checks did not execute.'
}
$transportProof = Get-Content -LiteralPath (Join-Path $script:Output 'prerequisites.json') -Raw | ConvertFrom-Json
if (@($transportProof.applications | Where-Object prerequisite -CEQ 'failure').Count -ne 2 -or
    @($transportProof.applications | Where-Object { $_.application -ceq 'p4kx' -and $_.prerequisite -ceq 'expected-refusal' }).Count -ne 1) {
    throw 'Valid CLI API4 refusal and failed MCP/GUI transport were not recorded separately.'
}
'PASS: 18 mocked calls; valid CLI; early paths; typed refusal/no output; two MCP/GUI transport failures fail the aggregate; 3 malformed outcomes rejected.'

& (Join-Path $PSScriptRoot 'test-flatpak-launchers.ps1')
