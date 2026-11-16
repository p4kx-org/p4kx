// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use p4kx_export::{Kind, SCHEMA_VERSION};
use serde::Deserialize;

use super::TestResult;
use super::inventory::Inventory;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Receipt {
    channel: String,
    archive_before: ArchiveStamp,
    archive_after: ArchiveStamp,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ArchiveStamp {
    manifest_sha256: String,
    build: MeasuredBuild,
    archive_bytes: u64,
    archive_created_utc: String,
    archive_modified_utc: String,
}

#[derive(Deserialize, PartialEq, Eq)]
struct MeasuredBuild {
    channel: String,
    #[serde(flatten)]
    build: Build,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Build {
    branch: String,
    version: String,
    change_number: String,
    #[serde(rename = "buildId")]
    id: String,
}

pub(super) struct Binding {
    channel: String,
    archive: ArchiveStamp,
}

impl Receipt {
    pub(super) fn read(directory: &Path) -> TestResult<Self> {
        let input = BufReader::new(File::open(directory.join("receipt.private.json"))?);
        Ok(serde_json::from_reader(input)?)
    }

    pub(super) fn binding(self) -> TestResult<Binding> {
        let archive = self.archive_before;
        if archive != self.archive_after || archive.build.channel != self.channel {
            return Err("measurement archive stamps or channels disagree".into());
        }
        if archive.manifest_sha256.len() != 64
            || !archive
                .manifest_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("invalid manifest digest in measurement stamp".into());
        }
        Ok(Binding {
            channel: self.channel,
            archive,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IndexedPass {
    schema_version: u32,
    source: CatalogSource,
    completeness: BTreeMap<String, Selection>,
    export_health: Publication,
    #[serde(rename = "provenance")]
    pub(super) inventory: Inventory,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogSource {
    channel: Option<String>,
    build: Option<Build>,
    archive_size: u64,
    archive_modified: Option<String>,
}

#[derive(Deserialize)]
struct Selection {
    status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Publication {
    #[serde(rename = "publicationMode")]
    mode: String,
    status: String,
    read: String,
    projection: String,
}

impl IndexedPass {
    pub(super) fn read(input: impl Read) -> serde_json::Result<Self> {
        serde_json::from_reader(input)
    }
}

impl Binding {
    pub(super) fn check(&self, pass: &IndexedPass) -> TestResult {
        if pass.schema_version != SCHEMA_VERSION {
            return Err("replay requires catalog schema v3".into());
        }
        let source = &pass.source;
        if source.channel.as_deref() != Some(self.channel.as_str())
            || source.build.as_ref() != Some(&self.archive.build.build)
            || source.archive_size != self.archive.archive_bytes
            || source.archive_modified.as_deref()
                != Some(stamp_seconds(&self.archive.archive_modified_utc)?.as_str())
        {
            return Err("catalog source identity differs from the measurement".into());
        }
        let publication = &pass.export_health;
        if publication.mode != "normal"
            || [
                &publication.status,
                &publication.read,
                &publication.projection,
            ]
            .iter()
            .any(|status| status.as_str() != "complete")
        {
            return Err("replay requires a complete normal publication".into());
        }
        if pass
            .completeness
            .values()
            .any(|selection| selection.status != "complete")
            || Kind::ALL
                .iter()
                .any(|kind| !pass.completeness.contains_key(kind.name()))
        {
            return Err("replay requires a complete unfiltered selection".into());
        }
        Ok(())
    }
}

fn stamp_seconds(stamp: &str) -> TestResult<String> {
    let utc = stamp
        .strip_suffix('Z')
        .ok_or("archive modification UTC suffix")?;
    let (seconds, fractional) = utc.split_once('.').unwrap_or((utc, "0"));
    if seconds.len() != 19
        || fractional.is_empty()
        || !fractional.bytes().all(|digit| digit.is_ascii_digit())
    {
        return Err("archive modification stamp is not a UTC timestamp".into());
    }
    Ok(format!("{seconds}Z"))
}
