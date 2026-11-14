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
    assert_eq!(rows.len(), 5, "required native targets");
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
    assert_eq!(
        rows.iter()
            .filter(|row| row.platform()
                == Platform {
                    os: OperatingSystem::Macos,
                    arch: Architecture::Arm64
                })
            .count(),
        1,
        "one Apple Silicon macOS row"
    );
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
    if cfg!(all(
        any(target_os = "windows", target_os = "linux"),
        any(target_arch = "x86_64", target_arch = "aarch64")
    )) || cfg!(all(target_os = "macos", target_arch = "aarch64"))
    {
        let current = current.expect("supported compiled target");
        assert_eq!(
            current.os == OperatingSystem::Windows,
            cfg!(target_os = "windows")
        );
        assert_eq!(
            current.os == OperatingSystem::Macos,
            cfg!(target_os = "macos"),
            "the compiled target determines macOS support"
        );
        assert_eq!(
            current.arch == Architecture::Arm64,
            cfg!(target_arch = "aarch64")
        );
    } else {
        assert_eq!(current, None);
    }
}

#[test]
fn macos_assets_and_binary_identity_follow_the_single_apple_silicon_row()
-> Result<(), Box<dyn std::error::Error>> {
    let macos = Platform {
        os: OperatingSystem::Macos,
        arch: Architecture::Arm64,
    };
    assert_eq!(
        asset_filename("0.5.0", macos, PackageKind::Dmg)?.as_deref(),
        Some("P4KX-0.5.0-aarch64-apple-darwin.dmg"),
        "canonical Apple Silicon DMG asset"
    );
    assert_eq!(
        asset_filename("0.5.0", macos, PackageKind::TarGz)?,
        None,
        "Linux package format is unavailable on macOS"
    );
    assert_eq!(
        asset_filename(
            "0.5.0",
            Platform {
                arch: Architecture::Amd64,
                ..macos
            },
            PackageKind::Dmg
        )?,
        None,
        "Intel macOS has no native package"
    );
    let source = include_str!("../../../.config/platforms.json");
    for (field, wrong) in [
        ("target", serde_json::json!("x86_64-apple-darwin")),
        ("runner_os", serde_json::json!("Darwin")),
        ("runner_arch", serde_json::json!("X64")),
        ("mach_cpu_type", serde_json::json!(0x0100_0007)),
        ("deployment_target", serde_json::json!("14.0")),
        ("pe_machine", serde_json::json!(0xaa64)),
    ] {
        let mut changed: Value = serde_json::from_str(source)?;
        let row = changed
            .get_mut("targets")
            .and_then(Value::as_array_mut)
            .and_then(|rows| {
                rows.iter_mut()
                    .find(|row| row.get("os").and_then(Value::as_str) == Some("macos"))
            })
            .and_then(Value::as_object_mut)
            .ok_or("missing macOS target row")?;
        row.insert(field.to_owned(), wrong);
        assert!(parse_manifest(&changed.to_string()).is_err(), "{field}");
    }
    Ok(())
}
