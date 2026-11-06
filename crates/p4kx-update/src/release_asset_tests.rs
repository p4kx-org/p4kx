// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::select_asset;
use crate::{
    Architecture, OperatingSystem, PackageKind, Platform, TargetDefinition, asset_filename,
    supported_targets,
};

const VERSION: &str = "0.5.0";

fn asset(name: &str) -> Value {
    json!({
        "name": name,
        "browser_download_url": format!("https://github.com/p4kx-org/p4kx/releases/download/v{VERSION}/{name}")
    })
}

fn assets_for<'a>(
    targets: impl IntoIterator<Item = &'a TargetDefinition>,
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut assets = Vec::new();
    for target in targets {
        for package in &target.packages {
            let name = asset_filename(VERSION, target.platform(), *package)?
                .ok_or("unsupported package")?;
            assets.push(asset(&name));
        }
    }
    Ok(Value::Array(assets))
}

#[test]
fn every_native_platform_and_package_selects_only_its_compatible_asset()
-> Result<(), Box<dyn std::error::Error>> {
    let targets = supported_targets()?;
    let assets = assets_for(&targets)?;
    for target in targets {
        for package in &target.packages {
            let selected = select_asset(VERSION, target.platform(), *package, &assets)?
                .ok_or("asset missing")?;
            assert_eq!(selected.target, target.target);
            assert_eq!(selected.package, *package);
            assert_eq!(
                Some(selected.name),
                asset_filename(VERSION, target.platform(), *package)?
            );
        }
    }
    Ok(())
}

#[test]
fn assets_for_another_architecture_do_not_match() -> Result<(), Box<dyn std::error::Error>> {
    let targets = supported_targets()?;
    for target in &targets {
        let assets = assets_for(
            targets
                .iter()
                .filter(|other| other.platform() != target.platform()),
        )?;
        for package in &target.packages {
            assert_eq!(
                select_asset(VERSION, target.platform(), *package, &assets)?,
                None
            );
        }
    }
    Ok(())
}

#[test]
fn only_windows_amd64_accepts_its_historical_asset_names() -> Result<(), Box<dyn std::error::Error>>
{
    let legacy = json!([
        asset("P4KX-0.5.0-x86_64.msi"),
        asset("P4KX-0.5.0-x86_64-portable.zip")
    ]);
    for target in supported_targets()? {
        for package in &target.packages {
            let selected = select_asset(VERSION, target.platform(), *package, &legacy)?;
            assert_eq!(
                selected.is_some(),
                target.os == OperatingSystem::Windows && target.arch == Architecture::Amd64
            );
        }
    }
    Ok(())
}

#[test]
fn malformed_duplicate_or_untrusted_assets_fall_back_to_the_release_page()
-> Result<(), Box<dyn std::error::Error>> {
    let platform = Platform {
        os: OperatingSystem::Windows,
        arch: Architecture::Arm64,
    };
    let name = "P4KX-0.5.0-aarch64-pc-windows-msvc.msi";
    for assets in [
        Value::Null,
        json!({}),
        json!([]),
        json!([{"name": name}]),
        json!([{"name":name, "browser_download_url":"javascript:bad"}]),
        json!([{"name":name, "browser_download_url":"https://example.invalid/package.msi"}]),
        json!([asset(name), asset(name)]),
    ] {
        assert_eq!(
            select_asset(VERSION, platform, PackageKind::Msi, &assets)?,
            None
        );
    }
    Ok(())
}

#[test]
fn canonical_assets_take_precedence_and_wrong_package_os_is_rejected()
-> Result<(), Box<dyn std::error::Error>> {
    let platform = Platform {
        os: OperatingSystem::Windows,
        arch: Architecture::Amd64,
    };
    let name = "P4KX-0.5.0-x86_64-pc-windows-msvc.msi";
    let assets = json!([asset(name), asset("P4KX-0.5.0-x86_64.msi")]);
    assert_eq!(
        select_asset(VERSION, platform, PackageKind::Msi, &assets)?
            .ok_or("missing asset")?
            .name,
        name
    );
    assert_eq!(
        select_asset(VERSION, platform, PackageKind::Deb, &assets)?,
        None
    );
    Ok(())
}
