param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$ArtifactDirectory,
    [Parameter(Mandatory)][string]$BuildDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'platform-contract.ps1')
$platform = Get-P4kxPlatform -Target $Target
Assert-P4kxNativeHost -Platform $platform
if ($platform.os -cne 'linux' -or $platform.arch -cne 'arm64') {
    throw 'The Debian Blender consumer requires a native Linux ARM64 host.'
}
$daemon = @(& docker info --format '{{.OSType}}/{{.Architecture}}')
if ($LASTEXITCODE -ne 0 -or $daemon.Count -ne 1 -or $daemon[0] -cnotin @('linux/aarch64', 'linux/arm64')) {
    throw 'The Blender fixture requires a native Linux ARM64 Docker daemon.'
}
if (-not [IO.Path]::IsPathFullyQualified($ArtifactDirectory) -or
    -not (Test-Path -LiteralPath $ArtifactDirectory -PathType Container) -or
    -not [IO.Path]::IsPathFullyQualified($BuildDirectory) -or (Test-Path -LiteralPath $BuildDirectory)) {
    throw 'The Debian consumer requires the downloaded packages and a new absolute build directory.'
}

$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
[void][IO.Directory]::CreateDirectory($BuildDirectory)
Copy-Item -LiteralPath (Join-Path $repository '.github/blender-arm64/Dockerfile') -Destination $BuildDirectory
Copy-Item -LiteralPath '/etc/ssl/certs/ca-certificates.crt' -Destination $BuildDirectory
$image = 'p4kx-blender-headless-arm64'
docker build --platform linux/arm64 --tag $image $BuildDirectory
if ($LASTEXITCODE -ne 0) { throw 'The pinned Debian ARM64 Blender image failed to build.' }
$imagePlatform = @(& docker image inspect --format '{{.Os}}/{{.Architecture}}' $image)
if ($LASTEXITCODE -ne 0 -or $imagePlatform.Count -ne 1 -or $imagePlatform[0] -cne 'linux/arm64') {
    throw 'The Blender consumer image is not native Linux ARM64.'
}

$dockerOptions = @(
    '--rm', '--platform', 'linux/arm64', '--network', 'none', '--read-only', '--tmpfs', '/tmp',
    '--env', 'GITHUB_SHA', '--env', 'GITHUB_RUN_ID', '--env', 'P4KX_PLATFORM_MATRIX',
    '--env', 'HOME=/tmp', '--env', 'PYTHONDONTWRITEBYTECODE=1',
    '--env', 'BLENDER_USER_CONFIG=/tmp/blender-config',
    '--env', 'BLENDER_USER_SCRIPTS=/tmp/blender-scripts', '--env', 'BLENDER_USER_DATAFILES=/tmp/blender-data',
    '--env', 'XDG_CACHE_HOME=/tmp/blender-cache',
    '--mount', "type=bind,source=$repository,target=/p4kx,readonly",
    '--mount', "type=bind,source=$ArtifactDirectory,target=/artifacts,readonly",
    $image, 'blender', '--background', '--factory-startup', '--disable-autoexec', '--threads', '2',
    '--python-exit-code', '1'
)
docker run @dockerOptions --python /p4kx/blender_addon/tests/blender_smoke.py
if ($LASTEXITCODE -ne 0) { throw 'Native Debian ARM64 Blender smoke test failed.' }
docker run @dockerOptions --python /p4kx/blender_addon/tests/verify_portable_packages.py -- /artifacts --target $Target --bpy-version 5.2.2
if ($LASTEXITCODE -ne 0) { throw 'Native Debian ARM64 Blender package exchange failed.' }
