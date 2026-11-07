<#
.SYNOPSIS
Verifies native build-environment selection and refusal of untrusted settings.
#>
param([Parameter(Mandatory)][string]$Directory, [string]$Target)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
$platform = if ($Target) { Get-P4kxPlatform $Target } else { Get-P4kxCurrentPlatform }
Assert-P4kxNativeHost $platform
$checker = Join-Path $PSScriptRoot 'check-build-tools.ps1'
$folder = [IO.Path]::GetFullPath($Directory)
if (Test-Path -LiteralPath $folder) { throw 'Build-environment test directory must be new.' }
New-Item -ItemType Directory -Path $folder -Force | Out-Null
$file = Join-Path $folder 'github-env.txt'
$sentinel = 'UNCHANGED=synthetic' + [Environment]::NewLine

function Assert-P4kxEnvironmentRefusal {
    param([string]$Checker, [hashtable]$Arguments, [string]$File, [string]$Expected)
    $rejected = $false
    try { & $Checker @Arguments | Out-Null } catch { $rejected = $true }
    if (-not $rejected -or [IO.File]::ReadAllText($File) -cne $Expected) {
        throw 'An invalid environment request succeeded or changed the output file.'
    }
}

$variables = @('GITHUB_ENV', 'GITHUB_OUTPUT', 'NATIVE_TOOLCHAIN')
$original = @{}
foreach ($name in $variables) { $original[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
try {
    $env:GITHUB_OUTPUT = $null
    $env:NATIVE_TOOLCHAIN = "untrusted`nINJECTED=1"
    $pins = @(& $checker -Target $platform.target)
    $native = @($pins | Where-Object { $_.StartsWith('rust-toolchain=') })
    if ($native.Count -ne 1) { throw 'Exactly one native repository toolchain is required.' }
    $linkers = @{
        'x86_64-pc-windows-msvc'='CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=rust-lld.exe'
        'aarch64-pc-windows-msvc'='CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_LINKER=rust-lld.exe'
        'x86_64-unknown-linux-gnu'='RUSTFLAGS=-Clink-arg=-fuse-ld=mold'
        'aarch64-unknown-linux-gnu'='RUSTFLAGS=-Clink-arg=-fuse-ld=mold'
    }
    $expected = @{
        toolchain=('RUSTUP_TOOLCHAIN=' + $native[0].Substring('rust-toolchain='.Length))
        linker=$linkers[$platform.target]
    }
    $env:GITHUB_ENV = $file
    foreach ($mode in @('toolchain', 'linker', 'ToolChain', 'LINKER')) {
        [IO.File]::WriteAllText($file, $sentinel, [Text.UTF8Encoding]::new($false))
        $emitted = @(& $checker -Target $platform.target -SelectEnvironment $mode)
        $text = [Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes($file))
        if ($emitted.Count -ne 0 -or $text -cne ($sentinel + $expected[$mode] + [Environment]::NewLine)) {
            throw 'Native environment selection changed existing data, trusted values or the output contract.'
        }
    }
    [IO.File]::WriteAllText($file, $sentinel, [Text.UTF8Encoding]::new($false))
    $foreign = @((Get-P4kxPlatformContract).targets | Where-Object { $_.target -cne $platform.target })[0]
    foreach ($arguments in @(
        @{Target=($platform.target + "`nINJECTED=1");SelectEnvironment='toolchain'},
        @{Target=$platform.target.ToUpperInvariant();SelectEnvironment='linker'},
        @{Target='unknown-native-target';SelectEnvironment='toolchain'},
        @{Target=$foreign.target;SelectEnvironment='linker'},
        @{Target=$platform.target;SelectEnvironment="linker`nINJECTED=1"},
        @{Target=$platform.target;SelectEnvironment='toolchain';Verify=$true},
        @{Target=$platform.target;SelectEnvironment='linker';Nextest=$true}
    )) {
        Assert-P4kxEnvironmentRefusal $checker $arguments $file $sentinel
    }
    foreach ($invalidFile in @('', 'relative-github-env.txt')) {
        $env:GITHUB_ENV = $invalidFile
        Assert-P4kxEnvironmentRefusal $checker @{Target=$platform.target;SelectEnvironment='toolchain'} $file $sentinel
    }
}
finally {
    foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $original[$name], 'Process') }
}
'PASS: exact native toolchain/linker settings, append preservation, no output leakage and nine refused injection/host/mode/file requests.'
