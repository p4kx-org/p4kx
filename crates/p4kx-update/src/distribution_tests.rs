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
