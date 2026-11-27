param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macos-dmg-mount.ps1')

function Invoke-P4kxMacTool {
    param([string]$Tool, [string[]]$Arguments)
    switch -CaseSensitive ($Tool) {
        '/usr/bin/hdiutil' {
            $mountPoint = [Array]::IndexOf($Arguments, '-mountpoint')
            if ($Arguments[0] -cne 'attach' -or '-readonly' -cnotin $Arguments -or $mountPoint -lt 0) {
                throw 'The fixture expects a read-only attach.'
            }
            $script:fixture.mount = $Arguments[$mountPoint + 1]
            $null = New-Item -ItemType Directory -Path (Join-Path $script:fixture.mount 'Blender.app')
            'fixture attached'
        }
        '/usr/bin/ditto' {
            if ($null -ne $script:fixture.copyError) { throw $script:fixture.copyError }
            $null = New-Item -ItemType Directory -Path $Arguments[1]
        }
        default { throw "Unexpected native tool: $Tool" }
    }
}

function Invoke-P4kxPortalProcess {
    param([string]$Executable, [string[]]$Arguments, [string]$Directory, [string]$Name, [int]$Milliseconds)
    $attempt = ++$script:fixture.attempts
    if ($Executable -cne '/usr/bin/hdiutil' -or $Arguments.Count -ne 2 -or $Arguments[0] -cne 'detach' -or
        $Arguments[1] -cne $script:fixture.mount -or $Directory -cne $script:fixture.directory -or
        $Name -cne "dmg-detach-$attempt" -or $Milliseconds -ne 20000 -or $attempt -gt $script:fixture.exitCodes.Count) {
        throw 'Unexpected detach invocation.'
    }
    $code = $script:fixture.exitCodes[$attempt - 1]
    if ($null -eq $code) { throw [TimeoutException]::new('Detach watchdog expired.') }
    @{ exit_code = $code; stdout = 'fixture detached'; stderr = 'fixture detach detail' }
}

function Start-Sleep {
    param([int]$Milliseconds)
    $script:fixture.delays.Add($Milliseconds)
}

$null = New-Item -ItemType Directory -Path $Directory
$copyError = [Management.Automation.ErrorRecord]::new(
    [IO.IOException]::new('Original copy failure.'), 'FixtureCopyFailed',
    [Management.Automation.ErrorCategory]::WriteError, 'fixture bundle'
)
$cases = @(
    @{ name = 'clean'; exitCodes = @(0); delays = @() }
    @{ name = 'transient-busy'; exitCodes = @(16, 0); delays = @(500) }
    @{ name = 'persistent-busy'; exitCodes = @(16, 16, 16); delays = @(500, 1000); cleanupError = 'exit 16' }
    @{ name = 'other-error'; exitCodes = @(13); delays = @(); cleanupError = 'exit 13' }
    @{ name = 'watchdog'; exitCodes = @($null); delays = @(); cleanupError = 'Detach watchdog expired' }
    @{ name = 'copy-failure'; exitCodes = @(0); delays = @(); copyError = $copyError }
    @{ name = 'dual-failure'; exitCodes = @(13); delays = @(); copyError = $copyError; cleanupError = 'exit 13' }
)
foreach ($case in $cases) {
    $script:fixture = @{
        directory = Join-Path $Directory $case.name
        mount = $null
        exitCodes = $case.exitCodes
        attempts = 0
        delays = [Collections.Generic.List[int]]::new()
        copyError = $case.copyError
    }
    $null = New-Item -ItemType Directory -Path $script:fixture.directory
    $relocated = $null
    $failure = $null
    try {
        $relocated = Copy-P4kxMacMountedBundle -Dmg 'fixture.dmg' -TaskDirectory $script:fixture.directory -BundleName 'Blender.app'
    } catch {
        $failure = $_
    }
    if ($script:fixture.attempts -ne $case.exitCodes.Count -or
        ($script:fixture.delays -join ',') -cne ($case.delays -join ',')) {
        throw "Wrong detach retry policy for $($case.name)."
    }
    if ($case.copyError) {
        if ($failure.FullyQualifiedErrorId -cne $copyError.FullyQualifiedErrorId -or
            ![object]::ReferenceEquals($failure.Exception, $copyError.Exception) -or
            $failure.TargetObject -cne $copyError.TargetObject) {
            throw "Copy error identity changed for $($case.name)."
        }
    } elseif ($case.cleanupError) {
        if ($failure.Exception.Message -cnotlike "*$($case.cleanupError)*") {
            throw "Detach failure was lost for $($case.name)."
        }
    } elseif ($failure -or !$relocated -or !(Test-Path -LiteralPath $relocated -PathType Container)) {
        throw "The verified bundle was not returned for $($case.name)."
    }
    if ($case.exitCodes[-1] -eq 0 -and
        (Get-Content -LiteralPath (Join-Path $script:fixture.directory 'detach.log') -Raw) -cne 'fixture detached') {
        throw "Successful detach log was lost for $($case.name)."
    }
}
'macOS DMG detach contract passed.'
