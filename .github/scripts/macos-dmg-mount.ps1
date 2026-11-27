$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macos-bundle.ps1')
. (Join-Path $PSScriptRoot 'macos-resources.ps1')
. (Join-Path $PSScriptRoot 'portal-process.ps1')

function Dismount-P4kxMacDmg {
    param([string]$Mount, [string]$TaskDirectory)
    $busyExitCode = 16
    $retryDelays = @(0, 500, 1000)
    for ($index = 0; $index -lt $retryDelays.Count; $index++) {
        $delay = $retryDelays[$index]
        if ($delay -gt 0) { Start-Sleep -Milliseconds $delay }
        $attempt = $index + 1
        $name = "dmg-detach-$attempt"
        $result = Invoke-P4kxPortalProcess -Executable /usr/bin/hdiutil -Arguments @('detach', $Mount) `
            -Directory $TaskDirectory -Name $name -Milliseconds 20000
        if ($result.exit_code -eq 0) {
            Set-Content -LiteralPath (Join-Path $TaskDirectory 'detach.log') -Value $result.stdout -Encoding utf8NoBOM -NoNewline
            return
        }
        if ($result.exit_code -ne $busyExitCode -or $attempt -eq $retryDelays.Count) {
            throw "DMG detach failed with exit $($result.exit_code); see $name-stderr.log."
        }
    }
}

function Copy-P4kxMacMountedBundle {
    param(
        [Parameter(Mandatory)][string]$Dmg,
        [Parameter(Mandatory)][string]$TaskDirectory,
        [ValidateSet('P4KX.app', 'Blender.app')][string]$BundleName = 'P4KX.app'
    )
    $mount = Join-Path $TaskDirectory 'mount'
    $install = Join-Path $TaskDirectory 'relocated package'
    $null = New-Item -ItemType Directory -Path $mount, $install
    $source = Join-Path $mount $BundleName
    $destination = Join-Path $install $BundleName
    $attached = $false
    $bodyFailed = $false
    try {
        $mountResult = @(Invoke-P4kxMacTool /usr/bin/hdiutil @('attach', '-readonly', '-nobrowse', '-noautoopen', '-mountpoint', $mount, $Dmg))
        $attached = $true
        $mountResult | Set-Content -LiteralPath (Join-Path $TaskDirectory 'attach.log') -Encoding utf8NoBOM
        $app = Get-Item -LiteralPath $source
        if ($app -isnot [IO.DirectoryInfo] -or $app.LinkType) {
            throw "Mounted DMG is missing the regular $BundleName bundle."
        }
        if ($BundleName -ceq 'P4KX.app') {
            $link = Get-Item -LiteralPath (Join-Path $mount 'Applications')
            if ($link.LinkType -cne 'SymbolicLink' -or $link.LinkTarget -cne '/Applications') {
                throw 'Mounted P4KX DMG is missing its Applications link.'
            }
        }
        Invoke-P4kxMacTool /usr/bin/ditto @($source, $destination) | Out-Host
    } catch {
        $bodyFailed = $true
        throw
    } finally {
        try {
            if ($attached -or (Test-Path -LiteralPath $source -PathType Container)) {
                Dismount-P4kxMacDmg -Mount $mount -TaskDirectory $TaskDirectory
            }
        } catch {
            if (!$bodyFailed) { throw }
            Write-Warning "DMG cleanup also failed: $($_.Exception.Message)" -WarningAction Continue
        }
    }
    return $destination
}

function Assert-P4kxMacRelocatedBundle {
    param(
        [Parameter(Mandatory)][string]$Bundle,
        [Parameter(Mandatory)]$Platform,
        [Parameter(Mandatory)][string]$Version,
        [Parameter(Mandatory)]$Plan
    )
    $paths = Get-P4kxMacBundlePaths -Bundle $Bundle
    $core = ($Version -split '[-+]', 2)[0]
    $expected = [ordered]@{
        CFBundleExecutable = 'p4kx-gui'
        CFBundleIdentifier = 'io.github.p4kx-org.p4kx'
        CFBundleShortVersionString = $core
        CFBundleVersion = $core
        CFBundlePackageType = 'APPL'
        LSMinimumSystemVersion = $Platform.deployment_target
    }
    foreach ($key in $expected.Keys) {
        $value = @(Invoke-P4kxMacTool /usr/bin/plutil @('-extract', $key, 'raw', '-o', '-', $paths.plist))
        if ($value.Count -ne 1 -or $value[0] -cne $expected[$key]) {
            throw "Relocated app bundle has inconsistent $key."
        }
    }
    Assert-P4kxMacSignature -Path $Bundle -Plan $Plan -Bundle -ExecutableCode
    foreach ($name in @('p4kx', 'p4kx-gui', 'p4kx-mcp')) {
        $binary = Join-Path $paths.binaries $name
        Assert-P4kxMacSignature -Path $binary -Plan $Plan -ExecutableCode
        Assert-P4kxMacSystemLibraries -Binary $binary
    }
    return $paths
}
