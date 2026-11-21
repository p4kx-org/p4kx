$ErrorActionPreference = 'Stop'

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
