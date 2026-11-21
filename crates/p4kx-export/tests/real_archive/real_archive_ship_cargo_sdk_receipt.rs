// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use p4kx_extraction::Archive;
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::TestResult;

const RECEIPT_VERSION: u8 = 1;

#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Stamp {
    channel: String,
    branch: String,
    version: String,
    build_id: String,
    change_number: String,
    archive_bytes: u64,
    archive_modified_nanos: u128,
}

impl Stamp {
    pub(super) fn read(archive: &Archive) -> TestResult<Self> {
        let build = archive
            .build()
            .ok_or("the cargo SDK witness needs a complete Build")?;
        let channel = p4kx_channels::channel_name(archive.path())
            .filter(|name| matches!(name.as_str(), "LIVE" | "PTU"))
            .ok_or("the cargo SDK witness requires LIVE or PTU")?;
        let expected = std::env::var("P4KX_CARGO_SDK_BUILD")
            .map_err(|_| "configure the expected installed Build changelist")?;
        if build.change_number() != expected
            || !expected.bytes().all(|byte| byte.is_ascii_digit())
            || [
                build.branch(),
                build.version(),
                build.build_id(),
                build.change_number(),
            ]
            .iter()
            .any(|field| field.trim().is_empty())
        {
            return Err("the Archive differs from the requested complete Build".into());
        }
        let modified = archive
            .file_modified()
            .ok_or("Archive modification time is unavailable")?;
        Ok(Self {
            channel,
            branch: build.branch().to_owned(),
            version: build.version().to_owned(),
            build_id: build.build_id().to_owned(),
            change_number: build.change_number().to_owned(),
            archive_bytes: archive.file_len(),
            archive_modified_nanos: modified.duration_since(UNIX_EPOCH)?.as_nanos(),
        })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Receipt {
    schema_version: u8,
    stamp: Stamp,
    record_index: u32,
    row_sha256: [u8; 32],
}

impl Receipt {
    pub(super) fn new(stamp: Stamp, record_index: u32, row: &Value) -> TestResult<Self> {
        Ok(Self {
            schema_version: RECEIPT_VERSION,
            stamp,
            record_index,
            row_sha256: fingerprint(row)?,
        })
    }

    pub(super) fn read(archive: &Archive) -> TestResult<Self> {
        let reader = File::open(destination()?)
            .map_err(|_| "open the verified private cargo SDK receipt")?;
        let receipt: Self = serde_json::from_reader(BufReader::new(reader))
            .map_err(|_| "read the completed private cargo SDK receipt")?;
        if receipt.schema_version != RECEIPT_VERSION || receipt.stamp != Stamp::read(archive)? {
            return Err(
                "the receipt differs from the expected schema or installed Archive stamp".into(),
            );
        }
        Ok(receipt)
    }

    pub(super) fn write(&self, path: &Path) -> TestResult {
        let mut output =
            File::create_new(path).map_err(|_| "create a private cargo SDK receipt")?;
        serde_json::to_writer(&mut output, self)
            .map_err(|_| "write the verified cargo SDK receipt")?;
        output
            .sync_all()
            .map_err(|_| "synchronize the verified cargo SDK receipt".into())
    }

    pub(super) const fn record_index(&self) -> u32 {
        self.record_index
    }

    pub(super) fn verify(&self, row: &Value) -> TestResult {
        if self.row_sha256 != fingerprint(row)? {
            return Err("the cold SDK result differs from the verified sourced row".into());
        }
        Ok(())
    }
}

fn fingerprint(row: &Value) -> TestResult<[u8; 32]> {
    Ok(Sha256::digest(serde_json::to_vec(row)?).into())
}

pub(super) fn destination() -> TestResult<PathBuf> {
    let path = std::env::var_os("P4KX_CARGO_SDK_RECEIPT")
        .map(PathBuf::from)
        .ok_or("configure the private cargo SDK receipt path")?;
    let dir = fs::canonicalize(std::env::temp_dir())
        .map_err(|_| "the configured temporary directory is unavailable")?;
    let parent = path
        .parent()
        .ok_or("a private cargo SDK receipt parent is required")?;
    let parent =
        fs::canonicalize(parent).map_err(|_| "prepare the private cargo SDK receipt parent")?;
    if !path.is_absolute() || !parent.starts_with(dir) || path.file_name().is_none() {
        return Err(
            "the cargo SDK receipt must be within the configured temporary directory".into(),
        );
    }
    Ok(path)
}

pub(super) fn open_archive() -> TestResult<Archive> {
    let path = real_archive_path().map_err(|_| "configure the original installed Archive")?;
    let key = real_archive_key().map_err(|_| "configure the installed Archive key")?;
    Archive::open_with_key(path, Some(key))
        .map_err(|_| "open the configured installed Archive".into())
}
