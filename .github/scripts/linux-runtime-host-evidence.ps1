# SPDX-License-Identifier: MIT OR Apache-2.0
# SPDX-FileCopyrightText: 2026 the P4KX contributors

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'linux-package-contract.ps1')

function Assert-P4kxLinuxDistribution($Identity) {
    $revision = '24.04'
    $prefix = [regex]::Escape($revision)
    if ($Identity.distribution_id -isnot [string] -or $Identity.distribution_id -cne 'ubuntu' -or
        $Identity.os_version_id -isnot [string] -or $Identity.os_version_id -cne $revision -or
        $Identity.os_version -isnot [string] -or
        $Identity.os_version -cnotmatch "\A$prefix(?:\.[0-9]+)?(?: [A-Za-z0-9()._-]+)*\z") {
        throw "Linux release packaging requires observed Ubuntu $revision distribution evidence."
    }
}

function ConvertFrom-P4kxLinuxOsRelease([string]$Text) {
    $keys = [ordered]@{ distribution_id = 'ID'; os_version_id = 'VERSION_ID'; os_version = 'VERSION' }
    $identity = [ordered]@{}
    foreach ($field in $keys.Keys) {
        $key = $keys[$field]
        $entries = [regex]::Matches($Text, "(?m)^$key=(?<literal>[^\r\n]*)\r?$")
        if ($entries.Count -ne 1 -or $entries[0].Groups['literal'].Value -cnotmatch
            '\A(?:"(?<value>[^"]+)"|''(?<value>[^'']+)''|(?<value>[A-Za-z0-9._-]+))\z') {
            throw "Linux os-release has missing, duplicate or malformed '$key' evidence."
        }
        $identity[$field] = $Matches.value
    }
    Assert-P4kxLinuxDistribution $identity
    return $identity
}

function Assert-P4kxLinuxRuntimeHostEvidence($Runtime) {
    $identity = $Runtime.runtime_host
    Assert-P4kxLinuxDistribution $identity
    $glibc = (Get-P4kxLinuxPackaging).maximum_glibc_requirement
    if ($Runtime.observed_glibc -isnot [string] -or $Runtime.observed_glibc -cne "glibc $glibc" -or
        $identity.kernel_release -isnot [string] -or
        $identity.kernel_release -cnotmatch '\A[0-9]+\.[0-9]+[A-Za-z0-9._+-]*\z') {
        throw 'Missing or incompatible native Linux glibc or kernel evidence.'
    }
    foreach ($field in @('image_os', 'image_version')) {
        $value = $identity.$field
        if ($null -ne $value -and ($value -isnot [string] -or $value -cnotmatch '\A[A-Za-z0-9][A-Za-z0-9._-]*\z')) {
            throw "Invalid optional Linux runner '$field' evidence."
        }
    }
}

function Get-P4kxLinuxRuntimeHostEvidence($Platform) {
    Assert-P4kxNativeHost -Platform $Platform
    $identity = ConvertFrom-P4kxLinuxOsRelease (Get-Content -LiteralPath '/etc/os-release' -Raw)
    $identity.kernel_release = Invoke-P4kxLinuxTool /usr/bin/uname @('-r')
    if ($env:ImageOS) { $identity.image_os = $env:ImageOS }
    if ($env:ImageVersion) { $identity.image_version = $env:ImageVersion }
    $runtime = [ordered]@{
        runtime_host = $identity
        observed_glibc = Invoke-P4kxLinuxTool /usr/bin/getconf @('GNU_LIBC_VERSION')
    }
    Assert-P4kxLinuxRuntimeHostEvidence $runtime
    return $runtime
}
