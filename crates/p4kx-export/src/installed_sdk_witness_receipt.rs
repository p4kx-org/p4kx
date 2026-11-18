// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use p4kx_extraction::Archive;
use serde::Serialize;
use sha2::Sha256;

use crate::catalog_queries::CatalogRecordCoverage;

use super::TestResult;
use super::selection::{Census, Family};
use super::verify::Evidence;

const MAX_RECEIPT_BYTES: usize = 4096;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Report {
    query: &'static str,
    owner_state: &'static str,
    declarations: usize,
    unreadable_declarations: usize,
    foreign_headers: usize,
    matching_headers: usize,
    outside_rule: usize,
    leftovers: usize,
    possible_unreadable: usize,
    admissible_owners: usize,
    rows_per_table: Vec<usize>,
    sources: usize,
    uses: usize,
    cells: usize,
    ancestor_links: usize,
    selected_coverage: [usize; 4],
    dependency_coverage: Option<[usize; 4]>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt<'report> {
    schema_version: u8,
    build_sha256: String,
    archive_bytes: u64,
    decoded_entries: usize,
    parsed_views: usize,
    requested_locales: usize,
    pre_cancel_probes: usize,
    after_callback_cancel_probe: bool,
    queries: &'report [Report],
}

impl Report {
    pub(super) fn new(
        family: Family,
        census: &Census,
        evidence: Evidence<'_>,
        rows_per_table: Vec<usize>,
        cells: usize,
    ) -> Self {
        Self {
            query: family.name(),
            owner_state: census.absence(),
            declarations: census.declarations,
            unreadable_declarations: census.unreadable_declarations,
            foreign_headers: census.foreign_types,
            matching_headers: census.matching_types,
            outside_rule: census.outside_rule,
            leftovers: census.leftovers,
            possible_unreadable: census.possible_unreadable,
            admissible_owners: census.owners.len(),
            rows_per_table,
            sources: evidence.inventory.source_count(),
            uses: evidence.inventory.use_count(),
            cells,
            ancestor_links: evidence
                .inventory
                .iter_sources()
                .map(|s| s.ancestors().len())
                .sum(),
            selected_coverage: coverage(evidence.coverage),
            dependency_coverage: evidence.dependencies.map(coverage),
        }
    }
}

fn coverage(value: &CatalogRecordCoverage) -> [usize; 4] {
    [
        value.unreadable_headers.len(),
        value.unreadable_bodies.len(),
        value.incomplete_fields.len(),
        value.skipped_leftovers,
    ]
}

pub(super) fn destination() -> TestResult<PathBuf> {
    let path = env::var_os("P4KX_SDK_WITNESS_FILE")
        .map(PathBuf::from)
        .ok_or("set P4KX_SDK_WITNESS_FILE to a new private scratch JSON file")?;
    let root = fs::canonicalize(env::temp_dir()).map_err(|_| "private scratch root unavailable")?;
    let parent = path.parent().ok_or("private receipt parent required")?;
    let parent =
        fs::canonicalize(parent).map_err(|_| "create the private receipt directory first")?;
    if !path.is_absolute()
        || !parent.starts_with(root)
        || path
            .try_exists()
            .map_err(|_| "private receipt destination check failed")?
    {
        return Err(
            "expected an unused absolute receipt path inside the configured temporary directory"
                .into(),
        );
    }
    Ok(path)
}

pub(super) fn write(path: &Path, archive: &Archive, reports: &[Report]) -> TestResult {
    use sha2::Digest as _;

    let build = archive.build().ok_or("installed Build identity required")?;
    let mut hasher = Sha256::new();
    for value in [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ] {
        hasher.update(u64::try_from(value.len())?.to_le_bytes());
        hasher.update(value.as_bytes());
    }
    let digest = hasher.finalize();
    let mut build_sha256 = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(build_sha256, "{byte:02x}")?;
    }
    let receipt = Receipt {
        schema_version: 1,
        build_sha256,
        archive_bytes: archive.file_len(),
        decoded_entries: 1,
        parsed_views: 1,
        requested_locales: 0,
        pre_cancel_probes: reports.len(),
        after_callback_cancel_probe: true,
        queries: reports,
    };
    let bytes = serde_json::to_vec(&receipt).map_err(|_| "private receipt serialization failed")?;
    if bytes.len() > MAX_RECEIPT_BYTES {
        return Err("private technical receipt exceeded its fixed size bound".into());
    }
    let mut output = File::create_new(path).map_err(|_| "private receipt create_new failed")?;
    output
        .write_all(&bytes)
        .map_err(|_| "private receipt write failed")?;
    output
        .sync_all()
        .map_err(|_| "private receipt synchronization failed")?;
    Ok(())
}
