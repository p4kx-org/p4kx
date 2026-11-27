// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::{Read as _, Write as _};
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::equipment::Loadout;
use p4kx_export::catalog_queries::equipment::resource_topology::ResourceTopologyPlacementSnapshot;
use p4kx_extraction::Archive;
use serde::{Deserialize, Serialize};

use super::super::bindings::Counts;
use super::TestResult;

const RECEIPT_VAR: &str = "P4KX_SDK_WITNESS_FILE";
const MAX_BYTES: u64 = 4 * 1024;
const RECEIPT_SCHEMA: u8 = 2;

#[derive(Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Stamp {
    branch: String,
    version: String,
    change_number: String,
    build_id: String,
    archive_bytes: u64,
    archive_modified_nanos: u128,
}

impl Stamp {
    pub(super) fn read(archive: &Archive) -> TestResult<Self> {
        let build = archive
            .build()
            .ok_or("topology witness needs a complete installed Build")?;
        if [
            build.branch(),
            build.version(),
            build.change_number(),
            build.build_id(),
        ]
        .into_iter()
        .any(|field| field.trim().is_empty())
        {
            return Err("the installed Build identity has empty fields".into());
        }
        let modified = archive
            .file_modified()
            .ok_or("Archive modification time is unavailable")?;
        Ok(Self {
            branch: build.branch().to_owned(),
            version: build.version().to_owned(),
            change_number: build.change_number().to_owned(),
            build_id: build.build_id().to_owned(),
            archive_bytes: archive.file_len(),
            archive_modified_nanos: modified.duration_since(UNIX_EPOCH)?.as_nanos(),
        })
    }

    pub(super) fn check_requested_change(&self) -> TestResult {
        let change = std::env::var("P4KX_TOPOLOGY_BUILD")
            .map_err(|_| "set the exact expected topology Build changelist")?;
        if self.change_number != change || !change.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("the Archive differs from the requested topology Build".into());
        }
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Report {
    schema_version: u8,
    stamp: Stamp,
    samples: [Sample; 2],
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Sample {
    pub(super) record_index: u32,
    rows: [usize; 3],
    loadout_issue_codes: Vec<String>,
    hardpoint_bindings: usize,
    room_bindings: usize,
    direct_relay_ports: usize,
    declaration_roles: usize,
    alignment_roles: usize,
    source_uses: usize,
    physical_sources: usize,
}

impl Report {
    pub(super) const fn new(stamp: Stamp, samples: [Sample; 2]) -> Self {
        Self {
            schema_version: RECEIPT_SCHEMA,
            stamp,
            samples,
        }
    }

    pub(super) fn publish(&self, path: &std::path::Path) -> TestResult {
        let bytes =
            serde_json::to_vec(self).map_err(|_| "serialize the topology witness receipt")?;
        if u64::try_from(bytes.len())? > MAX_BYTES {
            return Err("the topology receipt exceeds its size contract".into());
        }
        let mut output =
            File::create_new(path).map_err(|_| "create a new private topology receipt")?;
        output
            .write_all(&bytes)
            .map_err(|_| "write the topology witness receipt")?;
        output
            .sync_all()
            .map_err(|_| "synchronize the topology witness receipt")?;
        Ok(())
    }

    pub(super) fn sample(archive: &Archive) -> TestResult<Sample> {
        let path = destination()?;
        let file = File::open(path).map_err(|_| "open the completed private topology receipt")?;
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "read the completed private topology receipt")?;
        if u64::try_from(bytes.len())? > MAX_BYTES {
            return Err("the topology receipt exceeds its size contract".into());
        }
        let report: Self = serde_json::from_slice(&bytes)
            .map_err(|_| "the completed private topology receipt has an invalid format")?;
        if report.schema_version != RECEIPT_SCHEMA || report.stamp != Stamp::read(archive)? {
            return Err(
                "the topology receipt differs from the installed Build or Archive stamp".into(),
            );
        }
        report.stamp.check_requested_change()?;
        let [fixed, multiple_relays] = report.samples;
        match std::env::var("P4KX_TOPOLOGY_SAMPLE").as_deref() {
            Ok("0") => Ok(fixed),
            Ok("1") => Ok(multiple_relays),
            _ => Err("select one topology witness sample with P4KX_TOPOLOGY_SAMPLE=0 or 1".into()),
        }
    }
}

impl Sample {
    pub(super) fn new(
        record_index: u32,
        snapshot: &ResourceTopologyPlacementSnapshot,
        direct: &Counts,
        direct_relay_ports: usize,
    ) -> Self {
        Self {
            record_index,
            rows: rows(snapshot),
            loadout_issue_codes: issues(snapshot)
                .iter()
                .map(|code| (*code).to_owned())
                .collect(),
            hardpoint_bindings: direct.hardpoints,
            room_bindings: direct.rooms,
            direct_relay_ports,
            declaration_roles: snapshot.declaration_dependencies.len(),
            alignment_roles: snapshot.loadout_dependencies.alignment.len(),
            source_uses: snapshot.source_inventory.use_count(),
            physical_sources: snapshot.source_inventory.source_count(),
        }
    }

    pub(super) fn verify(&self, snapshot: &ResourceTopologyPlacementSnapshot) -> TestResult {
        if self.rows != rows(snapshot)
            || !self
                .loadout_issue_codes
                .iter()
                .map(String::as_str)
                .eq(issues(snapshot).iter().copied())
            || self.declaration_roles != snapshot.declaration_dependencies.len()
            || self.alignment_roles != snapshot.loadout_dependencies.alignment.len()
            || self.source_uses != snapshot.source_inventory.use_count()
            || self.physical_sources != snapshot.source_inventory.source_count()
        {
            return Err(
                "the cold query differs from the verified topology sample inventory".into(),
            );
        }
        Ok(())
    }
}

fn issues(snapshot: &ResourceTopologyPlacementSnapshot) -> &[&'static str] {
    snapshot.loadout.as_ref().map_or(&[], Loadout::issues)
}

fn rows(snapshot: &ResourceTopologyPlacementSnapshot) -> [usize; 3] {
    let placements = &snapshot.placements;
    [
        placements.resource_network_nodes().rows.len(),
        placements.resource_network_link_bindings().rows.len(),
        placements.room_association_bindings().rows.len(),
    ]
}

pub(super) fn destination() -> TestResult<PathBuf> {
    let path = std::env::var_os(RECEIPT_VAR)
        .map(PathBuf::from)
        .ok_or("set P4KX_SDK_WITNESS_FILE to the private topology receipt path")?;
    let directory = fs::canonicalize(std::env::temp_dir())
        .map_err(|_| "the configured temporary directory is unavailable")?;
    let parent = path
        .parent()
        .ok_or("a private topology receipt parent is required")?;
    let parent = fs::canonicalize(parent)
        .map_err(|_| "prepare the private topology receipt parent first")?;
    if !path.is_absolute() || !parent.starts_with(directory) {
        return Err(
            "the private topology receipt must be inside the configured temporary directory".into(),
        );
    }
    Ok(path)
}
