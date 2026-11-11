function Assert-P4kxMacosHostEvidence {
    param([Parameter(Mandatory)]$HostEvidence, [Parameter(Mandatory)]$Platform)
    if ($Platform.os -cne 'macos' -or $HostEvidence.architecture -cne 'arm64' -or
        $HostEvidence.os_version -cnotmatch '^\d+\.\d+(?:\.\d+)?$' -or
        $HostEvidence.os_build -cnotmatch '^[0-9]+[A-Z][0-9]+[A-Za-z]*$' -or
        [version]$HostEvidence.os_version -lt [version]$Platform.deployment_target) {
        throw 'Missing or incompatible native macOS host evidence.'
    }
}

function Get-P4kxMacosHostEvidence {
    param([Parameter(Mandatory)]$Platform)
    Assert-P4kxNativeHost -Platform $Platform
    $product = @(& /usr/bin/sw_vers -productVersion)
    if ($LASTEXITCODE -ne 0 -or $product.Count -ne 1) { throw 'macOS product version query failed.' }
    $build = @(& /usr/bin/sw_vers -buildVersion)
    if ($LASTEXITCODE -ne 0 -or $build.Count -ne 1) { throw 'macOS build version query failed.' }
    $evidence = [ordered]@{
        os_version = $product[0]
        os_build = $build[0]
        architecture = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString().ToLowerInvariant()
    }
    Assert-P4kxMacosHostEvidence -HostEvidence $evidence -Platform $Platform
    return $evidence
}
