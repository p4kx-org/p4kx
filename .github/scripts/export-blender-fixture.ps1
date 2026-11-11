param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)][string]$RunId
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'native-evidence.ps1')
. (Join-Path $PSScriptRoot 'binary-machine.ps1')

$platform = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
if ($RunId -cnotmatch '^[0-9]+$' -or ($env:GITHUB_RUN_ID -and $env:GITHUB_RUN_ID -cne $RunId)) {
    throw 'Blender fixture producer requires the current workflow run.'
}
if (-not [IO.Path]::IsPathFullyQualified($Directory) -or (Test-Path -LiteralPath $Directory)) {
    throw 'Blender fixture producer requires a new absolute task directory.'
}

function Get-PackageFileHashes {
    param([Parameter(Mandatory)][string]$Root)
    foreach ($entry in Get-ChildItem -LiteralPath $Root -Force -Recurse) {
        if ($entry.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw 'Generated Blender fixture contains a linked asset.'
        }
        if ($entry.PSIsContainer) { continue }
        $path = [IO.Path]::GetRelativePath($Root, $entry.FullName).Replace([char]92, [char]47)
        [ordered]@{
            path = $path
            sha256 = (Get-FileHash -LiteralPath $entry.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    }
}

function Export-Packages {
    param(
        [Parameter(Mandatory)][string]$Executable,
        [Parameter(Mandatory)][string]$FixtureRoot,
        [Parameter(Mandatory)][string]$PackageRoot
    )
    [void][IO.Directory]::CreateDirectory($PackageRoot)
    $syntheticKey = Join-Path $FixtureRoot 'synthetic.key'
    & $Executable --json --key-file $syntheticKey entity package (Join-Path $FixtureRoot 'assembly.p4k') TEST_Ship --out (Join-Path $PackageRoot 'assembly') | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Synthetic assembly export failed.' }
    & $Executable --json --key-file $syntheticKey entity package (Join-Path $FixtureRoot 'livery.p4k') TEST_Livery --texture-mip 2 --out (Join-Path $PackageRoot 'livery') | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Synthetic livery export failed.' }
}

$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$fixtures = Join-Path $Directory 'fixtures'
$artifact = Join-Path $Directory 'artifact'
$packages = Join-Path $artifact 'packages'
$repeat = Join-Path $Directory 'repeat'
$env:P4KX_FOLDERS_ROOT = Join-Path $Directory 'profile'
$env:P4KX_LAUNCHER_DIR = Join-Path $Directory 'launcher'
$env:P4KX_GAME_CHECK = 'off'
$env:TMP = Join-Path $Directory 'tmp'
$env:TEMP = $env:TMP
$env:TMPDIR = $env:TMP
foreach ($name in 'P4KX_ARCHIVE_KEY', 'P4KX_KEY_FILE') {
    Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue
}
foreach ($folder in $env:P4KX_LAUNCHER_DIR, $env:TMP) {
    [void][IO.Directory]::CreateDirectory($folder)
}

Push-Location -LiteralPath $repository
try {
    $revision = @(& git rev-parse HEAD)
    if ($LASTEXITCODE -ne 0 -or $revision.Count -ne 1 -or
        $revision[0] -cnotmatch '^(?:[0-9a-f]{40}|[0-9a-f]{64})$' -or
        ($env:GITHUB_SHA -and $env:GITHUB_SHA -cne $revision[0])) {
        throw 'Blender fixture producer requires the checked-out workflow revision.'
    }
    cargo run --locked --target $Target -p p4kx-testkit --example blender_consumer_fixture -- $fixtures
    if ($LASTEXITCODE -ne 0) { throw 'Synthetic Blender source fixture generation failed.' }
    cargo build --locked --target $Target -p p4kx --bin p4kx
    if ($LASTEXITCODE -ne 0) { throw 'Native P4KX CLI build failed.' }
    $metadata = & cargo metadata --locked --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) { throw 'Native CLI target directory query failed.' }
    $targetDirectory = ($metadata | ConvertFrom-Json).target_directory
    $name = if ($platform.os -ceq 'windows') { 'p4kx.exe' } else { 'p4kx' }
    $executable = Join-Path $targetDirectory "$Target/debug/$name"
    if ((Get-ImageMachine -Path $executable -Os $platform.os) -ne (Get-P4kxNativeMachine -Platform $platform)) {
        throw "The fixture exporter is not native to $Target."
    }
    Export-Packages -Executable $executable -FixtureRoot $fixtures -PackageRoot $packages
    Export-Packages -Executable $executable -FixtureRoot $fixtures -PackageRoot $repeat

    $files = @(Get-PackageFileHashes -Root $packages | Sort-Object -Property { $_.path } -CaseSensitive)
    $repeatFiles = @(Get-PackageFileHashes -Root $repeat | Sort-Object -Property { $_.path } -CaseSensitive)
    if ($files.Count -eq 0 -or ($files | ConvertTo-Json -Compress -AsArray) -cne
        ($repeatFiles | ConvertTo-Json -Compress -AsArray)) {
        throw "Repeated package exports differ on $Target."
    }

    [ordered]@{
        schema = 1
        revision = $revision[0]
        run_id = $RunId
        target = $Target
        os = $platform.os
        arch = $platform.arch
        exporter_sha256 = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).Hash.ToLowerInvariant()
        files = $files
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $artifact 'receipt.json') -Encoding utf8NoBOM
    Write-Output "Produced $($files.Count) deterministic Blender fixture files on $Target."
} finally {
    Pop-Location
}
