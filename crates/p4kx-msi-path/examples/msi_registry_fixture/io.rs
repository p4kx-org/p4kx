// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};

use p4kx_msi_path::native_fixture::FixtureGuard;
use serde::Serialize;
use serde::de::DeserializeOwned;

const MAX_JSON_BYTES: u64 = 40 << 20;

pub(super) fn read<T: DeserializeOwned>(guard: &FixtureGuard, name: &OsStr) -> io::Result<T> {
    use std::os::windows::fs::MetadataExt as _;
    const REPARSE_POINT: u32 = 0x400;
    let path = child(guard, name)?;
    let metadata = std::fs::symlink_metadata(&path)?;
    if !metadata.is_file()
        || metadata.file_attributes() & REPARSE_POINT != 0
        || metadata.len() > MAX_JSON_BYTES
    {
        return Err(invalid());
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_JSON_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).map_err(|_| invalid())? > MAX_JSON_BYTES {
        return Err(invalid());
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

pub(super) fn create(guard: &FixtureGuard, name: &OsStr) -> io::Result<BufWriter<File>> {
    let path = child(guard, name)?;
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    Ok(BufWriter::new(file))
}

pub(super) fn write(mut writer: BufWriter<File>, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut writer, value).map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    writer
        .into_inner()
        .map_err(io::IntoInnerError::into_error)?
        .sync_all()
}

fn child(guard: &FixtureGuard, name: &OsStr) -> io::Result<PathBuf> {
    let mut components = Path::new(name).components();
    if !matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    ) || !name.to_str().is_some_and(valid_name)
    {
        return Err(invalid());
    }
    Ok(guard.root().join(name))
}

#[expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "Evidence filenames require a canonical lowercase extension."
)]
fn valid_name(name: &str) -> bool {
    if !name.ends_with(".json")
        || name.len() > 128
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return false;
    }
    let base = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !base.is_empty()
        && !matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !(base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && base
                .as_bytes()
                .last()
                .is_some_and(|byte| (b'1'..=b'9').contains(byte)))
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "fixture evidence must be a bounded regular file inside its task directory",
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn evidence_names_exclude_streams_devices_and_path_aliases() {
        for name in [
            "NUL.json",
            "COM1.json",
            "LPT9.json",
            "a.json:stream",
            "../a.json",
            "a\\b.json",
            ".json",
            "a.json ",
        ] {
            assert!(!super::valid_name(name));
        }
        for name in ["baseline.json", "raw-path-0.json", "after_prepare.json"] {
            assert!(super::valid_name(name));
        }
    }
}
