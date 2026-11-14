$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'macos-signing.ps1')

function Get-P4kxMacSigningKeychains {
    foreach ($line in @(Invoke-P4kxMacTool /usr/bin/security @('list-keychains', '-d', 'user'))) {
        if ($line -cnotmatch '^\s*"(?<path>[^"\r\n]+)"\s*$' -or -not [IO.Path]::IsPathFullyQualified($Matches.path)) {
            throw 'Unexpected keychain search list.'
        }
        $Matches.path
    }
}

function Import-P4kxMacSigningCertificate {
    param(
        [string]$Path,
        [string]$Keychain,
        [string]$KeychainPassword
    )
    [IO.File]::WriteAllBytes($Path, [Convert]::FromBase64String($env:MACOS_CERTIFICATE_P12_BASE64))
    Invoke-P4kxMacTool /usr/bin/security @('import', $Path, '-P', $env:MACOS_CERTIFICATE_PASSWORD,
        '-T', '/usr/bin/codesign', '-t', 'cert', '-f', 'pkcs12', '-k', $Keychain) | Out-Null
    Invoke-P4kxMacTool /usr/bin/security @('set-key-partition-list', '-S', 'apple-tool:,apple:',
        '-s', '-k', $KeychainPassword, $Keychain) | Out-Null
    $found = @(foreach ($line in @(Invoke-P4kxMacTool /usr/bin/security @('find-identity', '-v', '-p', 'codesigning', $Keychain))) {
        if ($line -cmatch '^\s*\d+\) (?<fingerprint>[0-9A-Fa-f]{40}) "Developer ID Application: [^"\r\n]+"\s*$') {
            $Matches.fingerprint
        }
    })
    if ($found.Count -ne 1) { throw 'The temporary keychain must contain one valid Developer ID Application identity.' }
    return $found[0]
}

function Save-P4kxMacNotaryProfile {
    param([string]$Path, [string]$Keychain)
    $issuer = [Guid]::Empty
    if ($env:MACOS_NOTARY_KEY_ID -cnotmatch '^[A-Z0-9]{10}$' -or
        -not [Guid]::TryParseExact($env:MACOS_NOTARY_ISSUER_ID, 'D', [ref]$issuer) -or $issuer -eq [Guid]::Empty) {
        throw 'The App Store Connect API key identifiers are invalid.'
    }
    $profile = 'p4kx-release-notary'
    [IO.File]::WriteAllBytes($Path, [Convert]::FromBase64String($env:MACOS_NOTARY_KEY_BASE64))
    Invoke-P4kxMacTool /usr/bin/xcrun @('notarytool', 'store-credentials', $profile, '--keychain', $Keychain,
        '--key', $Path, '--key-id', $env:MACOS_NOTARY_KEY_ID, '--issuer', $issuer.ToString('D')) | Out-Null
    return $profile
}

function Invoke-P4kxMacDmgSigning {
    param(
        [Parameter(Mandatory)][hashtable]$Arguments,
        [Parameter(Mandatory)][string]$CredentialDirectory,
        [switch]$DryRun
    )
    $builder = Join-Path $PSScriptRoot 'build-macos-dmg.ps1'
    if ($DryRun) {
        & $builder @Arguments -DryRun
        return
    }
    if ($env:GITHUB_EVENT_NAME -cne 'push' -or $env:GITHUB_REF_TYPE -cne 'tag' -or
        $env:GITHUB_REPOSITORY -cne 'p4kx-org/p4kx' -or $env:GITHUB_REF_NAME -cne "v$($Arguments.Version)") {
        throw 'Developer ID package mode requires the matching repository release tag.'
    }
    $certificate = @($env:MACOS_CERTIFICATE_P12_BASE64, $env:MACOS_CERTIFICATE_PASSWORD)
    $notary = @($env:MACOS_NOTARY_KEY_BASE64, $env:MACOS_NOTARY_KEY_ID, $env:MACOS_NOTARY_ISSUER_ID)
    $certCount = @($certificate | Where-Object { $_ }).Count
    $notaryCount = @($notary | Where-Object { $_ }).Count
    if (($certCount -ne 0 -and $certCount -ne $certificate.Count) -or
        ($notaryCount -ne 0 -and $notaryCount -ne $notary.Count)) {
        throw 'Configured macOS certificate or notary credentials are incomplete.'
    }
    if ($certCount -eq 0 -and $notaryCount -eq 0) {
        & $builder @Arguments
        return
    }
    $null = New-Item -ItemType Directory -Path $CredentialDirectory
    Invoke-P4kxMacTool /bin/chmod @('700', $CredentialDirectory) | Out-Null
    $keychain = Join-Path $CredentialDirectory 'signing.keychain-db'
    $certPath = Join-Path $CredentialDirectory 'certificate.p12'
    $keyPath = Join-Path $CredentialDirectory 'notary-key.p8'
    $password = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
    $previous = @()
    $restore = $false
    $created = $false
    try {
        Invoke-P4kxMacTool /usr/bin/security @('create-keychain', '-p', $password, $keychain) | Out-Null
        $created = $true
        Invoke-P4kxMacTool /usr/bin/security @('set-keychain-settings', '-lut', '21600', $keychain) | Out-Null
        Invoke-P4kxMacTool /usr/bin/security @('unlock-keychain', '-p', $password, $keychain) | Out-Null
        $signing = @{}
        if ($certCount -gt 0) {
            $signing.SigningIdentity = Import-P4kxMacSigningCertificate -Path $certPath -Keychain $keychain -KeychainPassword $password
            $previous = @(Get-P4kxMacSigningKeychains | Where-Object { $_ -cne $keychain })
            $restore = $true
            Invoke-P4kxMacTool /usr/bin/security (@('list-keychains', '-d', 'user', '-s', $keychain) + $previous) | Out-Null
        }
        if ($notaryCount -gt 0) {
            $signing.NotaryProfile = Save-P4kxMacNotaryProfile -Path $keyPath -Keychain $keychain
            $signing.NotaryKeychain = $keychain
        }
        & $builder @Arguments @signing
    }
    finally {
        try {
            if ($restore) {
                Invoke-P4kxMacTool /usr/bin/security (@('list-keychains', '-d', 'user', '-s') + $previous) | Out-Null
            }
        }
        finally {
            try {
                if ($created) { Invoke-P4kxMacTool /usr/bin/security @('delete-keychain', $keychain) | Out-Null }
            }
            finally {
                try { [IO.File]::Delete($certPath) }
                finally { [IO.File]::Delete($keyPath) }
            }
        }
    }
}
