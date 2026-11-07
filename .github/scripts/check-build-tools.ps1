param(
    [switch]$Verify,
    [switch]$Nextest,
    [string]$Target
)

$ErrorActionPreference = 'Stop'
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$toolchainPath = Join-Path $repoRoot 'rust-toolchain.toml'
$toolchain = Get-Content -LiteralPath $toolchainPath -Raw
$channels = [regex]::Matches($toolchain, '(?m)^\s*channel\s*=\s*"(?<version>\d+\.\d+\.\d+)"\s*$')
if ($channels.Count -ne 1) {
    throw 'rust-toolchain.toml must select one exact MAJOR.MINOR.PATCH compiler release.'
}
$rustVersion = $channels[0].Groups['version'].Value
$nextestVersion = (Get-Content -LiteralPath (Join-Path $repoRoot '.config/nextest-version') -Raw).Trim()
if ($nextestVersion -cnotmatch '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$') {
    throw '.config/nextest-version must select one exact MAJOR.MINOR.PATCH release.'
}

. (Join-Path $PSScriptRoot 'platform-contract.ps1')
$platform = if ($Target) { Get-P4kxPlatform -Target $Target } else { Get-P4kxCurrentPlatform }
Assert-P4kxNativeHost -Platform $platform
$nativeToolchain = "$rustVersion-$($platform.target)"
$outputs = @("rust-version=$rustVersion", "nextest-version=$nextestVersion", "rust-toolchain=$nativeToolchain", "target=$($platform.target)")

if (-not $Verify) {
    if ($env:GITHUB_OUTPUT) {
        $outputs | Add-Content -LiteralPath $env:GITHUB_OUTPUT -Encoding utf8NoBOM
    }
    $outputs
    return
}

$compiler = & rustc --version --verbose
if ($LASTEXITCODE -ne 0) { throw 'rustc version query failed.' }
$compiler
if ("release: $rustVersion" -cnotin $compiler -or "host: $($platform.target)" -cnotin $compiler) {
    throw "The compiler does not match the repository pin $rustVersion."
}
& cargo --version
if ($LASTEXITCODE -ne 0) { throw 'cargo version query failed.' }

if ($Nextest) {
    $runner = & cargo nextest --version
    if ($LASTEXITCODE -ne 0) { throw 'cargo-nextest version query failed.' }
    $runner
    $match = [regex]::Match(($runner -join [Environment]::NewLine), '(?m)^cargo-nextest (?<version>[^\s]+)')
    if (-not $match.Success -or $match.Groups['version'].Value -cne $nextestVersion -or "host: $($platform.target)" -cnotin $runner) {
        throw "The test runner does not match the repository pin $nextestVersion."
    }
}

$outputs
