// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::{
    Architecture, OperatingSystem, PackageKind, Platform, asset_filename, parse_manifest,
    supported_targets,
};

#[test]
fn unsafe_versions_and_colliding_package_names_are_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let platform = Platform {
        os: OperatingSystem::Windows,
        arch: Architecture::Amd64,
    };
    assert!(asset_filename("../../outside", platform, PackageKind::Msi).is_err());
    let mut source: Value = serde_json::from_str(include_str!("../../../.config/platforms.json"))?;
    source["packages"]["deb"] = Value::String(".tar.gz".to_owned());
    assert!(parse_manifest(&source.to_string()).is_err());
    Ok(())
}

#[test]
fn the_contract_contains_each_required_native_platform_once()
-> Result<(), Box<dyn std::error::Error>> {
    let rows = supported_targets()?;
    assert_eq!(rows.len(), 4);
    for os in [OperatingSystem::Windows, OperatingSystem::Linux] {
        for arch in [Architecture::Amd64, Architecture::Arm64] {
            assert_eq!(
                rows.iter()
                    .filter(|row| row.platform() == Platform { os, arch })
                    .count(),
                1
            );
        }
    }
    Ok(())
}

#[test]
fn missing_duplicate_and_mismatched_targets_are_rejected() -> Result<(), Box<dyn std::error::Error>>
{
    let source = include_str!("../../../.config/platforms.json");
    let mut missing: Value = serde_json::from_str(source)?;
    missing["targets"]
        .as_array_mut()
        .ok_or("missing targets")?
        .pop();
    assert!(parse_manifest(&missing.to_string()).is_err());

    let mut duplicate: Value = serde_json::from_str(source)?;
    duplicate["targets"][1] = duplicate["targets"][0].clone();
    assert!(parse_manifest(&duplicate.to_string()).is_err());

    for (field, wrong) in [
        ("target", "aarch64-unknown-linux-gnu"),
        ("runner_arch", "ARM64"),
        ("msi_arch", "arm64"),
        ("runner_os", "Linux"),
    ] {
        let mut mismatch: Value = serde_json::from_str(source)?;
        mismatch["targets"][0][field] = Value::String(wrong.to_owned());
        assert!(parse_manifest(&mismatch.to_string()).is_err(), "{field}");
    }
    Ok(())
}

#[test]
fn missing_and_wrong_os_packages_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let mut source: Value = serde_json::from_str(include_str!("../../../.config/platforms.json"))?;
    source["targets"][0]["packages"] = serde_json::json!(["msi", "deb"]);
    assert!(parse_manifest(&source.to_string()).is_err());
    source["targets"][0]["packages"] = serde_json::json!(["msi", "msi"]);
    assert!(parse_manifest(&source.to_string()).is_err());
    Ok(())
}

#[test]
fn the_running_target_uses_compile_time_architecture() {
    let current = Platform::current();
    if cfg!(any(target_os = "windows", target_os = "linux"))
        && cfg!(any(target_arch = "x86_64", target_arch = "aarch64"))
    {
        let current = current.expect("supported compiled target");
        assert_eq!(
            current.os == OperatingSystem::Windows,
            cfg!(target_os = "windows")
        );
        assert_eq!(
            current.arch == Architecture::Arm64,
            cfg!(target_arch = "aarch64")
        );
    } else {
        assert_eq!(current, None);
    }
}
