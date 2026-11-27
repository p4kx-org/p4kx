// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::fs::{self, File};
use std::io::{Read as _, Write as _};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::crafting;
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use super::{SHIP_INDEX, open_archive, optional_index, original, verified_host_rows};
use crate::real_archive_catalog_table_parity::{Origins, TestResult};
use crate::real_archive_verification::print_line;

const NOMINATION_PATH: &str = "P4KX_CRAFTER_WITNESS_NOMINATION";
const RECEIPT_VERSION: u8 = 1;
const MAX_RECEIPT_BYTES: u64 = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Nomination {
    version: u8,
    archive: ArchiveStamp,
    ship_record_index: u32,
    mounts: original::Mounts,
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ArchiveStamp {
    channel: String,
    branch: String,
    version: String,
    build_id: String,
    change_number: String,
    manifest_sha256: [u8; 32],
    archive_bytes: u64,
    archive_modified_nanos: u128,
}

impl ArchiveStamp {
    fn read(archive: &Archive) -> TestResult<Self> {
        let build = archive.build().ok_or("the archive needs a Build")?;
        let channel = p4kx_channels::channel_name(archive.path())
            .filter(|name| matches!(name.as_str(), "LIVE" | "PTU"))
            .ok_or("the Crafter host witness requires LIVE or PTU")?;
        let manifest = fs::read(archive.path().with_file_name("build_manifest.id"))
            .map_err(|_| "read the original Build manifest")?;
        let modified = archive
            .file_modified()
            .ok_or("missing Archive modification time")?;
        Ok(Self {
            channel,
            branch: build.branch().to_owned(),
            version: build.version().to_owned(),
            build_id: build.build_id().to_owned(),
            change_number: build.change_number().to_owned(),
            manifest_sha256: Sha256::digest(manifest).into(),
            archive_bytes: archive.file_len(),
            archive_modified_nanos: modified.duration_since(UNIX_EPOCH)?.as_nanos(),
        })
    }
}

#[test]
#[ignore = "real Archive: record one complete original mounted Crafter outside the measured query"]
fn nominate_original_crafter_host() -> TestResult {
    let destination = destination()?;
    let archive = open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    let stamp = ArchiveStamp::read(&archive)?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the original DataForge Entry")?;
    let forge = entry.open().map_err(|_| "parse original DataForge")?;
    let host = if let Some(record_index) = optional_index(SHIP_INDEX)? {
        original::MountedHost {
            record_index,
            mounts: original::resolve_mounts(&archive, &forge, &watch, record_index)?,
        }
    } else {
        original::nominate_mounts(&archive, &forge, &watch)?
    };
    watch
        .check()
        .map_err(|_| "Archive changed during host nomination")?;
    if stamp != ArchiveStamp::read(&archive)? {
        return Err("the original Build or Archive stamp changed during host nomination".into());
    }
    let receipt = Nomination {
        version: RECEIPT_VERSION,
        archive: stamp,
        ship_record_index: host.record_index,
        mounts: host.mounts,
    };
    let bytes = serde_json::to_vec(&receipt)?;
    if u64::try_from(bytes.len())? > MAX_RECEIPT_BYTES {
        return Err("the Crafter host nomination exceeds its receipt size".into());
    }
    let mut file = File::create_new(destination).map_err(|_| "create a private host nomination")?;
    file.write_all(&bytes)
        .map_err(|_| "write the private host nomination")?;
    file.sync_all()
        .map_err(|_| "synchronize the private host nomination")?;
    print_line(&format!(
        "Original Crafter host nomination groups={} port routes={}",
        receipt.mounts.len(),
        receipt.mounts.values().map(Vec::len).sum::<usize>(),
    ));
    Ok(())
}

#[test]
#[ignore = "real Archive: replay a private original Crafter host nomination in a fresh process"]
fn installed_crafter_host_cold_query() -> TestResult {
    let mut bytes = Vec::new();
    File::open(destination()?)
        .map_err(|_| "open the private original host nomination")?
        .take(MAX_RECEIPT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "read the private original host nomination")?;
    if u64::try_from(bytes.len())? > MAX_RECEIPT_BYTES {
        return Err("the Crafter host nomination exceeds its receipt size".into());
    }
    let receipt: Nomination = serde_json::from_slice(&bytes)
        .map_err(|_| "decode the private original host nomination")?;
    let archive = open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    if receipt.version != RECEIPT_VERSION
        || receipt.archive != ArchiveStamp::read(&archive)?
        || receipt.mounts.is_empty()
        || receipt
            .mounts
            .values()
            .any(|routes| routes.is_empty() || routes.iter().any(Vec::is_empty))
    {
        return Err(
            "the host nomination differs from the expected Archive or lacks positive routes".into(),
        );
    }
    let snapshot = crafting::read_crafter_hosts(&archive, receipt.ship_record_index, None)
        .map_err(|_| "read the nominated Crafter host through the SDK")?;
    let rows = verified_host_rows(
        &archive,
        receipt.ship_record_index,
        &snapshot,
        &receipt.mounts,
        &mut Origins::default(),
    )?;
    watch
        .check()
        .map_err(|_| "Archive changed during the cold Crafter query")?;
    print_line(&format!(
        "Cold Crafter host query matched {} original mount groups and closed their source references",
        rows.len(),
    ));
    Ok(())
}

fn destination() -> TestResult<PathBuf> {
    env::var_os(NOMINATION_PATH)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| "configure an absolute private Crafter host nomination path".into())
}
