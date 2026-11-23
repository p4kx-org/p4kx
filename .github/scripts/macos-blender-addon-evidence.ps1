# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

. (Join-Path $PSScriptRoot 'platform-contract.ps1')
. (Join-Path $PSScriptRoot 'pinned-download.ps1')
. (Join-Path $PSScriptRoot 'macos-dmg-mount.ps1')
. (Join-Path $PSScriptRoot 'macos-host-evidence.ps1')
. (Join-Path $PSScriptRoot 'macos-release-evidence.ps1')

function Get-P4kxMacosBlenderPin {
    $version = '5.2.4'
    @{
        version = $version
        url = "https://download.blender.org/release/Blender5.2/blender-$version-macos-arm64.dmg"
        sha256 = 'e73d564f97061b2496ccc5a0bd738937d2d3d0acae23621a00bbc7534dfd135f'
    }
}

function Assert-P4kxMacosBlenderAddonEvidence {
    param($Runtime, [ValidateSet('minimum', 'current')][string]$Role, [string]$AddonArtifactId, [string]$AddonHash)

    $platform = Get-P4kxPlatform -Target 'aarch64-apple-darwin'
    Assert-P4kxMacosHostedRole $Runtime.runtime_host $platform $Role
    $pin = Get-P4kxMacosBlenderPin
    $evidence = $Runtime.blender
    if ($AddonArtifactId -cnotmatch '\A[1-9][0-9]*\z' -or $AddonHash -cnotmatch '\A[0-9a-f]{64}\z' -or
        $null -eq $evidence -or $evidence.scope -cne 'native-blender-synthetic' -or
        $evidence.role -cne $Role -or $evidence.revision -cne $Runtime.revision -or
        $evidence.workflow_run -cne $Runtime.workflow_run -or
        $evidence.workflow_attempt -cne $Runtime.workflow_attempt -or
        $evidence.workflow_attempt -cnotmatch '\A[1-9][0-9]*\z' -or
        $evidence.extension_artifact_id -cne $AddonArtifactId -or $evidence.extension_sha256 -cne $AddonHash -or
        $evidence.cpu -cne 'arm64' -or $evidence.os_version -cne $Runtime.runtime_host.os_version -or
        $evidence.os_build -cne $Runtime.runtime_host.os_build -or
        $evidence.blender_version -cne $pin.version -or
        $evidence.blender_distribution_sha256 -cne $pin.sha256 -or
        $evidence.blender_sha256 -cnotmatch '\A[0-9a-f]{64}\z' -or
        $evidence.extension_install -cne 'success' -or $evidence.installed_import -cne 'success') {
        throw "The $Role macOS smoke lacks matching native Blender 5.2 LTS acceptance for the exact add-on ZIP."
    }
}

function Read-P4kxMacosBlenderPackageRuntime {
    param([string]$Path, [string]$Version)

    $runtime = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json -AsHashtable
    if ($runtime.scope -cne 'native-synthetic' -or $runtime.revision -cne $env:GITHUB_SHA -or
        $runtime.version -cne $Version -or
        $runtime.workflow_run -cne "$env:GITHUB_SERVER_URL/$env:GITHUB_REPOSITORY/actions/runs/$env:GITHUB_RUN_ID") {
        throw 'The macOS Blender check requires the matching native package receipt.'
    }
    return $runtime
}

function Invoke-P4kxMacosBlenderAddon {
    param(
        [string]$RuntimePath,
        [ValidateSet('minimum', 'current')][string]$Role,
        [string]$Version,
        [string]$AddonDirectory,
        [ValidatePattern('\A[1-9][0-9]*\z')][string]$AddonArtifactId,
        [string]$TaskDirectory
    )

    $platform = Get-P4kxPlatform -Target 'aarch64-apple-darwin'
    $actualHost = Get-P4kxMacosHostEvidence -Platform $platform
    $runtime = Read-P4kxMacosBlenderPackageRuntime $RuntimePath $Version
    if ($actualHost.os_version -cne $runtime.runtime_host.os_version -or
        $actualHost.os_build -cne $runtime.runtime_host.os_build -or
        $runtime.workflow_attempt -cne $env:GITHUB_RUN_ATTEMPT) {
        throw 'The Blender check must run on the same native host and attempt as the DMG smoke.'
    }

    $extension = Get-P4kxReleaseBlenderExtension $AddonDirectory $Version
    $extensionHash = (Get-FileHash -LiteralPath $extension -Algorithm SHA256).Hash.ToLowerInvariant()
    $pin = Get-P4kxMacosBlenderPin
    $task = New-Item -ItemType Directory -Path $TaskDirectory
    $dmg = Join-Path $task.FullName 'blender.dmg'
    Save-P4kxPinnedDownload $pin.url $pin.sha256 $dmg
    $bundle = Copy-P4kxMacMountedBundle -Dmg $dmg -TaskDirectory $task.FullName -BundleName 'Blender.app'
    Invoke-P4kxMacTool /usr/bin/codesign @('--verify', '--deep', '--strict', $bundle) | Out-Host
    $blender = Get-Item -LiteralPath (Join-Path $bundle 'Contents/MacOS/Blender')
    if ($blender -isnot [IO.FileInfo] -or $blender.LinkType) {
        throw 'Expected the pinned Blender bundle executable.'
    }
    $architectures = @(Invoke-P4kxMacTool /usr/bin/lipo @('-archs', $blender.FullName))
    if ($architectures.Count -ne 1 -or $architectures[0] -cne 'arm64') {
        throw 'The pinned Blender bundle is not native ARM64.'
    }
    $script = Join-Path $PSScriptRoot '../../blender_addon/tests/native_extension_smoke.py'
    $smoke = Join-Path $task.FullName 'smoke'
    & python3 $script --blender $blender.FullName --zip $extension --work-root $smoke --revision $env:GITHUB_SHA
    if ($LASTEXITCODE -ne 0) { throw 'Native Blender extension acceptance failed.' }

    $receipt = Get-Content -LiteralPath (Join-Path $smoke 'result.json') -Raw | ConvertFrom-Json -AsHashtable
    $receipt.role = $Role
    $receipt.os_build = $actualHost.os_build
    $receipt.workflow_run = $runtime.workflow_run
    $receipt.workflow_attempt = $runtime.workflow_attempt
    $receipt.extension_artifact_id = $AddonArtifactId
    $receipt.blender_distribution_sha256 = $pin.sha256
    $runtime.blender = $receipt
    Assert-P4kxMacosBlenderAddonEvidence $runtime $Role $AddonArtifactId $extensionHash
    $runtime | ConvertTo-Json -Depth 32 | Set-Content -LiteralPath $RuntimePath -Encoding utf8NoBOM
    Write-Output "Verified the exact add-on ZIP in native Blender $($pin.version) on macOS $($actualHost.os_version)."
}

function Assert-P4kxMacosBlenderAddonSet {
    param([string]$EvidenceDirectory, [string]$AddonDirectory, [string]$Version, [string]$AddonArtifactId)

    $extension = Get-P4kxReleaseBlenderExtension $AddonDirectory $Version
    $hash = (Get-FileHash -LiteralPath $extension -Algorithm SHA256).Hash.ToLowerInvariant()
    foreach ($role in @('minimum', 'current')) {
        $runtime = Read-P4kxMacosBlenderPackageRuntime (Join-Path $EvidenceDirectory "$role.json") $Version
        Assert-P4kxMacosBlenderAddonEvidence $runtime $role $AddonArtifactId $hash
    }
}
