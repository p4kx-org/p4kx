// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use super::{Distribution, MAX_IDENTITY_BYTES};
use crate::{Architecture, OperatingSystem, Platform, supported_targets};

#[test]
fn each_package_identity_matches_only_the_compiled_binary_platform()
-> Result<(), Box<dyn std::error::Error>> {
    let rows = supported_targets()?;
    for row in &rows {
        for package in &row.packages {
            let bytes = serde_json::to_vec(&json!({"target":row.target, "package":package}))?;
            for binary in &rows {
                let identity = Distribution::parse(&bytes, binary.platform());
                assert_eq!(identity.is_some(), binary.platform() == row.platform());
            }
        }
    }
    Ok(())
}

#[test]
fn unknown_malformed_and_oversized_identity_uses_the_page_fallback() {
    let platform = Platform {
        os: OperatingSystem::Windows,
        arch: Architecture::Arm64,
    };
    let oversized = usize::try_from(MAX_IDENTITY_BYTES).expect("identity bound fits usize") + 1;
    for bytes in [
        b"not json".to_vec(),
        br#"{"target":"aarch64-pc-windows-msvc","package":"unknown"}"#.to_vec(),
        br#"{"target":"aarch64-pc-windows-msvc","package":"deb"}"#.to_vec(),
        vec![b' '; oversized],
    ] {
        assert_eq!(Distribution::parse(&bytes, platform), None);
    }
}

#[test]
fn a_relocated_macos_bundle_uses_resources_for_every_application()
-> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    let temporary = tempfile::tempdir()?;
    let contents = temporary.path().join("Relocated P4KX.app/Contents");
    let macos = contents.join("MacOS");
    let resources = contents.join("Resources");
    fs::create_dir_all(&macos)?;
    fs::create_dir_all(&resources)?;
    let platform = Platform {
        os: OperatingSystem::Macos,
        arch: Architecture::Arm64,
    };
    let expected = Some(Distribution {
        platform,
        package: crate::PackageKind::Dmg,
    });
    let identity = resources.join("p4kx-distribution.json");
    fs::write(
        &identity,
        br#"{"target":"aarch64-apple-darwin","package":"dmg"}"#,
    )?;
    for name in ["p4kx", "p4kx-gui", "p4kx-mcp"] {
        let executable = macos.join(name);
        fs::write(&executable, b"synthetic executable")?;
        assert_eq!(
            Distribution::for_executable(&executable, platform),
            expected
        );
        #[cfg(unix)]
        {
            let alias = temporary.path().join(name);
            std::os::unix::fs::symlink(&executable, &alias)?;
            assert_eq!(Distribution::for_executable(&alias, platform), expected);
        }
    }

    fs::rename(&identity, macos.join("p4kx-distribution.json"))?;
    assert_eq!(
        Distribution::for_executable(&macos.join("p4kx"), platform),
        None
    );
    Ok(())
}
