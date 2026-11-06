// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Embeds the Linux application identity and the Windows executable icon.

use std::error::Error;
use std::path::Path;

#[cfg(windows)]
const ICON: &str = "assets/p4kx.ico";

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    linux_identity()?;
    #[cfg(windows)]
    windows_icon()?;
    Ok(())
}

fn linux_identity() -> Result<(), Box<dyn Error>> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.config/linux-packaging.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let identity = value
        .get("application_id")
        .and_then(serde_json::Value::as_str)
        .filter(|identity| !identity.is_empty() && !identity.contains(['\r', '\n']))
        .ok_or("Linux packaging requires a valid application_id")?;
    println!("cargo:rustc-env=P4KX_LINUX_APP_ID={identity}");
    Ok(())
}

#[cfg(windows)]
fn windows_icon() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed={ICON}");
    if std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon(ICON);
        resource.compile()?;
    }
    Ok(())
}
