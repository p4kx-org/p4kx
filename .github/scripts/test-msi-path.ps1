param([Parameter(Mandatory)][string]$Directory)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'msi-path.ps1')
$root = [IO.Path]::GetFullPath($Directory)
if (Test-Path -LiteralPath $root) { throw 'MSI PATH test directory must be new.' }
New-Item -ItemType Directory -Path $root | Out-Null

$key = [pscustomobject]@{ Names=@('Path'); Raw='%USERPROFILE%\工具;' }
$key | Add-Member -MemberType ScriptMethod -Name GetValueNames -Value { return $this.Names }
$key | Add-Member -MemberType ScriptMethod -Name GetValueKind -Value {
    param($Name)
    if ($Name -cne 'Path') { throw 'The getter truncated the complete registry value name.' }
    return [Microsoft.Win32.RegistryValueKind]::ExpandString
}
$key | Add-Member -MemberType ScriptMethod -Name GetValue -Value {
    param($Name, $Default, $Options)
    if ($Name -cne 'Path' -or $null -ne $Default -or
        $Options -ne [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) { throw 'The getter expanded or renamed the original registry value.' }
    return $this.Raw
}
$captured = Get-P4kxPathStateFromKey $key 'C:\Synthetic\工具;'
if (-not $captured.exists -or $captured.registry_name -cne 'Path' -or
    $captured.registry_kind -cne 'ExpandString' -or $captured.raw_value -cne $key.Raw -or
    $captured.user_api_value -cne 'C:\Synthetic\工具;') { throw 'A single complete registry value was not captured exactly.' }
$key.Names = @('Other')
$missing = Get-P4kxPathStateFromKey $key $null
if ($missing.exists -or $null -ne $missing.raw_value) { throw 'An absent PATH was fabricated from another value.' }
$missingKey = Get-P4kxPathStateFromKey $null $null
if ($missingKey.exists -or $null -ne $missingKey.registry_kind) { throw 'An absent registry key was fabricated.' }
$key.Names = @('Path', 'PATH')
$ambiguous = $false
try { Get-P4kxPathStateFromKey $key $null | Out-Null } catch { $ambiguous = $true }
if (-not $ambiguous) { throw 'Ambiguous PATH value names were accepted.' }

function New-PathState {
    param($Raw, $Api = $Raw, [string]$Kind = 'ExpandString', [bool]$Exists = $true)
    return [pscustomobject]@{
        exists=$Exists; registry_name=$(if ($Exists) { 'Path' } else { $null })
        registry_kind=$(if ($Exists) { $Kind } else { $null })
        raw_value=$Raw; user_api_value=$Api
    }
}

function Assert-RejectedPathChange {
    param($Before, $After, [string]$Expected)
    if ((Get-P4kxPathDifference $Before $After) -cne $Expected) { throw 'PATH mutation was classified incorrectly.' }
    $rejected = $false
    try { Assert-P4kxPathRestored $Before $After } catch { $rejected = $true }
    if (-not $rejected) { throw 'An exact PATH mutation was accepted.' }
}

$original = New-PathState '%USERPROFILE%\工具;C:\Foreign\bin;' 'C:\Users\Synthetic\工具;C:\Foreign\bin;'
Assert-P4kxPathRestored $original $original
Assert-RejectedPathChange $original (New-PathState '%USERPROFILE%\工具;C:\Foreign\bin' 'C:\Users\Synthetic\工具;C:\Foreign\bin') 'rawValueChanged'
Assert-RejectedPathChange $original (New-PathState '%USERPROFILE%\工具;c:\foreign\bin;' $original.user_api_value) 'rawValueChanged'
Assert-RejectedPathChange $original (New-PathState $original.user_api_value $original.user_api_value) 'rawValueChanged'
Assert-RejectedPathChange $original (New-PathState $original.raw_value $original.user_api_value 'String') 'registryKindChanged'
Assert-RejectedPathChange $original (New-PathState $original.raw_value 'changed expansion') 'apiValueChanged'
$absent = New-PathState $null $null 'ExpandString' $false
$empty = New-PathState '' '' 'String'
Assert-P4kxPathRestored $absent $absent
Assert-P4kxPathRestored $empty $empty
Assert-RejectedPathChange $absent $empty 'existenceChanged'
Assert-RejectedPathChange $original (New-PathState ($original.raw_value + ';C:\Foreign\new') ($original.user_api_value + ';C:\Foreign\new')) 'rawValueChanged'

$artifact = Join-Path $root 'transaction.path.json'
Write-P4kxPathTransition $original $empty $artifact
$saved = Get-Content -LiteralPath $artifact -Raw | ConvertFrom-Json
if ($saved.schema_version -ne 1 -or $saved.difference -cne 'registryKindChanged' -or
    $saved.before.raw_value -cne $original.raw_value -or $saved.before.user_api_value -cne $original.user_api_value -or
    $saved.before.registry_kind -cne 'ExpandString' -or $saved.after.raw_value -cne '') {
    throw 'PATH diagnostic evidence lost raw spelling, expansion, kind or empty value.'
}
'PASS: exact PATH preservation and 7 rejected mutations; raw/expanded/registry-kind diagnostic round trip. No registry writes or installation.'
