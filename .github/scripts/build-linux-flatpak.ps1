param(
    [Parameter(Mandatory)][string]$Target,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$BinaryDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][string]$StagingDirectory
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-payload.ps1')
$platform = Get-P4kxLinuxPlatform -Target $Target
$settings = Get-P4kxLinuxPackaging
$name = Get-P4kxAssetName -Target $Target -Version $Version -Package flatpak
$staging = [IO.Path]::GetFullPath($StagingDirectory)
$payload = Join-Path $staging 'payload'
$original = @(New-P4kxLinuxPayload -Target $Target -Version $Version -Package flatpak -BinaryDirectory $BinaryDirectory -OutputDirectory $payload)
Invoke-P4kxLinuxTool flatpak @('remote-add', '--user', '--if-not-exists', 'flathub', $settings.flatpak.runtime_repository) | Out-Null
$runtime = "$($settings.flatpak.runtime)//$($settings.flatpak.runtime_branch)"
$sdk = "$($settings.flatpak.sdk)//$($settings.flatpak.runtime_branch)"
Invoke-P4kxLinuxTool flatpak @('install', '--user', '--assumeyes', '--noninteractive', "--arch=$($platform.unix_arch)", 'flathub', $runtime, $sdk) | Out-Null
$manifest = [ordered]@{
    id=$settings.application_id; runtime=$settings.flatpak.runtime; sdk=$settings.flatpak.sdk
    'runtime-version'=$settings.flatpak.runtime_branch; branch=$settings.flatpak.application_branch
    command='p4kx-gui'; 'finish-args'=@($settings.flatpak.finish_args)
    'build-options'=@{strip=$false; 'no-debuginfo'=$true}
    modules=@(@{
        name='p4kx'; buildsystem='simple'; 'build-commands'=@('cp -a bin share /app/')
        sources=@(@{type='dir'; path=$payload})
    })
}
$manifestPath = Join-Path $staging "$($settings.application_id).json"
$manifest | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $manifestPath -Encoding utf8NoBOM
$repository = Join-Path $staging 'repository'
$build = Join-Path $staging 'build'
Invoke-P4kxLinuxTool flatpak-builder @(
    '--user', '--force-clean', '--disable-rofiles-fuse', "--arch=$($platform.unix_arch)",
    "--state-dir=$(Join-Path $staging 'state')", "--repo=$repository", $build, $manifestPath
) | Out-Null
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$asset = [IO.Path]::GetFullPath((Join-Path $OutputDirectory $name))
Invoke-P4kxLinuxTool flatpak @(
    'build-bundle', "--arch=$($platform.unix_arch)", "--runtime-repo=$($settings.flatpak.runtime_repository)",
    $repository, $asset, $settings.application_id, $settings.flatpak.application_branch
) | Out-Null
$producerPath = Join-Path $OutputDirectory "$name.producer.json"
Write-P4kxLinuxPackageProducer -Target $Target -Version $Version -Package flatpak -Asset $asset -BinaryDirectory (Join-Path $build 'files/bin') -OriginalImages $original -Transformations @('flatpak-sdk-install') -Output $producerPath
$producer = Get-Content -LiteralPath $producerPath -Raw | ConvertFrom-Json
$runtimeRef = (Invoke-P4kxLinuxTool flatpak @('info', '--user', "--arch=$($platform.unix_arch)", '--show-ref', $runtime) | Out-String).Trim()
$runtimeCommit = (Invoke-P4kxLinuxTool flatpak @('info', '--user', "--arch=$($platform.unix_arch)", '--show-commit', $runtime) | Out-String).Trim()
$sdkRef = (Invoke-P4kxLinuxTool flatpak @('info', '--user', "--arch=$($platform.unix_arch)", '--show-ref', $sdk) | Out-String).Trim()
$sdkCommit = (Invoke-P4kxLinuxTool flatpak @('info', '--user', "--arch=$($platform.unix_arch)", '--show-commit', $sdk) | Out-String).Trim()
$appRef = "app/$($settings.application_id)/$($platform.unix_arch)/$($settings.flatpak.application_branch)"
$appCommit = (Invoke-P4kxLinuxTool ostree @("--repo=$repository", 'rev-parse', $appRef) | Out-String).Trim()
if ($runtimeRef -cne "runtime/$($settings.flatpak.runtime)/$($platform.unix_arch)/$($settings.flatpak.runtime_branch)" -or
    $sdkRef -cne "runtime/$($settings.flatpak.sdk)/$($platform.unix_arch)/$($settings.flatpak.runtime_branch)" -or
    $runtimeCommit -cnotmatch '^[0-9a-f]{64}$' -or $sdkCommit -cnotmatch '^[0-9a-f]{64}$' -or
    $appCommit -cnotmatch '^[0-9a-f]{64}$') { throw 'Flatpak application/runtime/SDK CPU or OSTree identity differs.' }
$producer | Add-Member -NotePropertyName flatpak -NotePropertyValue ([ordered]@{
    app_ref=$appRef; app_commit=$appCommit
    runtime_ref=$runtimeRef; runtime_commit=$runtimeCommit; sdk_ref=$sdkRef; sdk_commit=$sdkCommit
    permissions=@($settings.flatpak.finish_args)
})
$producer | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $producerPath -Encoding utf8NoBOM
