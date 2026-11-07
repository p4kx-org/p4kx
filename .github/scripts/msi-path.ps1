$ErrorActionPreference = 'Stop'

<#
.SYNOPSIS
Reads the user PATH without expanding or modifying its registry value.
#>
function Get-P4kxUserPathState {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $false)
    try {
        $names = if ($null -eq $key) { @() } else { @($key.GetValueNames() | Where-Object { $_ -ieq 'Path' }) }
        if ($names.Count -gt 1) { throw 'The user PATH has ambiguous registry value names.' }
        $exists = $names.Count -eq 1
        return [pscustomobject]@{
            exists = $exists
            registry_name = $(if ($exists) { $names[0] } else { $null })
            registry_kind = $(if ($exists) { $key.GetValueKind($names[0]).ToString() } else { $null })
            raw_value = $(if ($exists) { $key.GetValue($names[0], $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) } else { $null })
            user_api_value = [Environment]::GetEnvironmentVariable('Path', 'User')
        }
    }
    finally { if ($null -ne $key) { $key.Dispose() } }
}

<#
.SYNOPSIS
Classifies exact PATH preservation without normalising separators, spelling or expansion.
#>
function Get-P4kxPathDifference {
    param([Parameter(Mandatory)]$Before, [Parameter(Mandatory)]$After)
    if ($Before.exists -cne $After.exists) { return 'existenceChanged' }
    if ($Before.registry_kind -cne $After.registry_kind) { return 'registryKindChanged' }
    if ($Before.raw_value -cne $After.raw_value) { return 'rawValueChanged' }
    if ($Before.user_api_value -cne $After.user_api_value) { return 'apiValueChanged' }
    return 'unchanged'
}

<#
.SYNOPSIS
Writes exact before/after PATH evidence for one completed installer transaction.
#>
function Write-P4kxPathTransition {
    param([Parameter(Mandatory)]$Before, [Parameter(Mandatory)]$After, [Parameter(Mandatory)][string]$Output)
    [ordered]@{
        schema_version = 1
        difference = Get-P4kxPathDifference $Before $After
        before = $Before
        after = $After
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Output -Encoding utf8NoBOM
}

<#
.SYNOPSIS
Requires the exact original raw PATH, registry kind and user API result.
#>
function Assert-P4kxPathRestored {
    param([Parameter(Mandatory)]$Original, [Parameter(Mandatory)]$Current)
    $difference = Get-P4kxPathDifference $Original $Current
    if ($difference -cne 'unchanged') { throw "MSI did not restore the original user PATH ($difference)." }
}
