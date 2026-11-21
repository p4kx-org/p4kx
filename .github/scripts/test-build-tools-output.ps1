param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
$checker = Join-Path $PSScriptRoot 'check-build-tools.ps1'
$platform = Get-P4kxCurrentPlatform
$pins = @(& $checker -Target $platform.target)
$toolchain = ($pins | Where-Object { $_.StartsWith('rust-version=') }).Substring('rust-version='.Length)
$nextest = ($pins | Where-Object { $_.StartsWith('nextest-version=') }).Substring('nextest-version='.Length)
$tools = [IO.Path]::GetFullPath($Directory)
if (Test-Path -LiteralPath $tools) { throw 'Build-tool output test directory must be new.' }
New-Item -ItemType Directory -Path $tools -Force | Out-Null
@'
"rustc $env:P4KX_TEST_COMPILER_RELEASE (synthetic)"
"release: $env:P4KX_TEST_COMPILER_RELEASE"
"host: $env:P4KX_TEST_COMPILER_HOST"
exit ([int]$env:P4KX_TEST_COMPILER_EXIT)
'@ | Set-Content -LiteralPath (Join-Path $tools 'rustc.ps1') -Encoding utf8NoBOM
@'
param([string]$Command)
if ($Command -eq 'nextest') {
    "cargo-nextest $env:P4KX_TEST_RUNNER_RELEASE"
    "host: $env:P4KX_TEST_RUNNER_HOST"
    exit ([int]$env:P4KX_TEST_RUNNER_EXIT)
} else { 'cargo synthetic' }
exit 0
'@ | Set-Content -LiteralPath (Join-Path $tools 'cargo.ps1') -Encoding utf8NoBOM
$variables = @('PATH', 'GITHUB_OUTPUT', 'P4KX_TEST_COMPILER_RELEASE', 'P4KX_TEST_COMPILER_HOST', 'P4KX_TEST_RUNNER_RELEASE', 'P4KX_TEST_RUNNER_HOST', 'P4KX_TEST_COMPILER_EXIT', 'P4KX_TEST_RUNNER_EXIT')
$original = @{}
foreach ($name in $variables) { $original[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
try {
    $env:PATH = $tools + [IO.Path]::PathSeparator + $env:PATH
    $env:GITHUB_OUTPUT = Join-Path $tools 'github-output.txt'
    $env:P4KX_TEST_COMPILER_RELEASE = $toolchain
    $env:P4KX_TEST_COMPILER_HOST = $platform.target
    $env:P4KX_TEST_RUNNER_RELEASE = $nextest
    $env:P4KX_TEST_RUNNER_HOST = $platform.target
    $env:P4KX_TEST_COMPILER_EXIT = '0'
    $env:P4KX_TEST_RUNNER_EXIT = '0'
    foreach ($name in @('rustc', 'cargo')) {
        if ((Get-Command $name -CommandType ExternalScript).Source -cne (Join-Path $tools "$name.ps1")) { throw 'Synthetic build-tool interception did not apply.' }
    }
    $verified = @(& $checker -Verify -Nextest -Target $platform.target)
    foreach ($pin in $pins) {
        if (@($verified | Where-Object { $_ -ceq $pin }).Count -ne 1) { throw "Verified tool output did not contain exactly one canonical $pin." }
    }
    if (Test-Path -LiteralPath $env:GITHUB_OUTPUT) { throw 'Verify unexpectedly changed GitHub outputs.' }
    $compiler = ($verified | Where-Object { $_.StartsWith('rust-version=') }).Substring('rust-version='.Length)
    $runner = ($verified | Where-Object { $_.StartsWith('nextest-version=') }).Substring('nextest-version='.Length)
    if ($compiler -cne $toolchain -or $runner -cne $nextest) { throw 'Package producer cannot consume verified pins.' }
    $mutations = [ordered]@{
        P4KX_TEST_COMPILER_RELEASE='invalid-synthetic-value'; P4KX_TEST_COMPILER_HOST='invalid-synthetic-value'
        P4KX_TEST_RUNNER_RELEASE='invalid-synthetic-value'; P4KX_TEST_RUNNER_HOST='invalid-synthetic-value'
        P4KX_TEST_COMPILER_EXIT='2'; P4KX_TEST_RUNNER_EXIT='3'
    }
    foreach ($name in $mutations.Keys) {
        $expected = [Environment]::GetEnvironmentVariable($name, 'Process')
        [Environment]::SetEnvironmentVariable($name, $mutations[$name], 'Process')
        $emitted = [Collections.Generic.List[string]]::new()
        $rejected = $false
        try { & $checker -Verify -Nextest -Target $platform.target | ForEach-Object { $emitted.Add($_) } }
        catch { $rejected = $true }
        [Environment]::SetEnvironmentVariable($name, $expected, 'Process')
        if (-not $rejected -or @($emitted | Where-Object { $_ -cmatch '^(rust-version|nextest-version|rust-toolchain|target|cache-platform)=' }).Count -ne 0) {
            throw "Invalid $name was accepted or emitted canonical verified pins."
        }
    }
    $env:P4KX_TEST_RUNNER_EXIT = '3'
    $compilerOnly = @(& $checker -Verify -Target $platform.target)
    foreach ($pin in $pins) {
        if (@($compilerOnly | Where-Object { $_ -ceq $pin }).Count -ne 1) { throw 'Compiler-only verification changed the canonical pin contract.' }
    }
}
finally {
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $original[$name], 'Process') }
}
'PASS: verified canonical pins, producer consumption, optional runner, compiler/runner version/host/exit refusal. Synthetic tools only.'
