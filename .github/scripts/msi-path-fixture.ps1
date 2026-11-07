$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-installation.ps1')

function Assert-P4kxHostedPathFixture {
    if (-not $IsWindows -or $env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted') {
        throw 'PATH fixtures require a disposable GitHub-hosted Windows runner.'
    }
}

function Assert-P4kxPathFixtureTarget {
    param($Target)
    if ($Target.exists -isnot [bool]) { throw 'PATH fixture existence must be a boolean.' }
    if ($Target.exists) {
        if ($Target.registry_name -ine 'Path' -or $Target.registry_kind -cnotin @('String','ExpandString') -or
            $Target.raw_value -isnot [string]) { throw 'PATH fixture must be one string-valued user PATH.' }
    }
    elseif ($null -ne $Target.raw_value -or $null -ne $Target.registry_kind -or $null -ne $Target.registry_name) { throw 'An absent PATH fixture must have no value, name or registry kind.' }
}

function New-P4kxPathFixtureState {
    param([AllowEmptyString()][string]$Raw, [ValidateSet('String','ExpandString')][string]$Kind='ExpandString')
    return [pscustomobject]@{exists=$true; registry_name='Path'; registry_kind=$Kind; raw_value=$Raw}
}

function Get-P4kxPathFixtureCases {
    return @(
        [pscustomobject]@{name='expand_plain'; state=(New-P4kxPathFixtureState '%USERPROFILE%\.dotnet\tools;C:\P4kxSynthetic\foreign')},
        [pscustomobject]@{name='expand_leading'; state=(New-P4kxPathFixtureState ';%USERPROFILE%\.dotnet\tools;C:\P4kxSynthetic\foreign')},
        [pscustomobject]@{name='expand_trailing'; state=(New-P4kxPathFixtureState '%USERPROFILE%\.dotnet\tools;C:\P4kxSynthetic\foreign;')},
        [pscustomobject]@{name='expand_doubled'; state=(New-P4kxPathFixtureState ';;%USERPROFILE%\.dotnet\tools;;C:\P4kxSynthetic\foreign;;')},
        [pscustomobject]@{name='string_plain'; state=(New-P4kxPathFixtureState 'C:\P4kxSynthetic\first;C:\P4kxSynthetic\foreign' 'String')},
        [pscustomobject]@{name='string_empty'; state=(New-P4kxPathFixtureState '' 'String')},
        [pscustomobject]@{name='expand_empty'; state=(New-P4kxPathFixtureState '')},
        [pscustomobject]@{name='absent'; state=[pscustomobject]@{exists=$false; registry_name=$null; registry_kind=$null; raw_value=$null}}
    )
}

<#
.SYNOPSIS
Resets only a closed hosted-runner fixture's user PATH and records the separate mutation.
#>
function Set-P4kxPathFixture {
    param($Context, $Target, $Expected, [string]$Log)
    Assert-P4kxHostedPathFixture
    Assert-P4kxPathFixtureTarget $Target
    Assert-P4kxInstallationOwnershipAbsent $Context.upgrade $Context.folder $Context.shortcut
    $before = Get-P4kxUserPathState
    Assert-P4kxPathRestored $Expected $before
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
    if ($null -eq $key) { throw 'The hosted user Environment key is absent.' }
    try {
        if ($Target.exists) {
            $kind = if ($Target.registry_kind -ceq 'String') { [Microsoft.Win32.RegistryValueKind]::String } else { [Microsoft.Win32.RegistryValueKind]::ExpandString }
            $key.SetValue($Target.registry_name, $Target.raw_value, $kind)
        }
        else { $key.DeleteValue('Path', $false) }
    }
    finally { $key.Dispose() }
    $after = Get-P4kxUserPathState
    Write-P4kxPathTransition $before $after $Log
    if ($after.exists -cne $Target.exists -or $after.registry_kind -cne $Target.registry_kind -or
        $after.raw_value -cne $Target.raw_value) { throw 'The fixture reset did not write its exact declared raw state.' }
    return $after
}
