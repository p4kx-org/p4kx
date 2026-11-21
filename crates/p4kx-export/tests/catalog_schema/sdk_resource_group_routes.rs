// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ResourceGroupMembers,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry, SourcedValue};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use super::super::{
    RESOURCE_GROUP_MEMBERS, RESOURCE_GROUP_PARENTS, RESOURCE_GROUPS, RESOURCE_TYPES,
};
use super::{Route, TestResult};

#[path = "../it/canonical_json.rs"]
mod canonical_json;
#[path = "../it/resource_group_records.rs"]
mod fixture;

const REMAINING: &str =
    "typed query and synthetic producer parity witnessed; complete installed producer parity open";

pub(super) fn read() -> TestResult<[Route; 4]> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("resource_group_fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            r"Data\Game2.dcb",
            fixture::graph().build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let mut options = CatalogOptions::new(UNIX_EPOCH);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value =
        serde_json::from_slice(build_catalog_from(&archive, &entry, &options)?.json())?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let all = CatalogRecordSelection::All;
    let (groups, members, target) =
        parsed.with_resource_groups(all, None, |snapshot| -> TestResult<_> {
            Ok((
                witness(RESOURCE_GROUPS, snapshot.definitions(), &catalog)?,
                witness(RESOURCE_GROUP_MEMBERS, snapshot.members(), &catalog)?,
                member_target(snapshot.members())?,
            ))
        })??;
    let parents = parsed.with_resource_group_parents(all, None, |snapshot| {
        witness(RESOURCE_GROUP_PARENTS, snapshot.rows(), &catalog)
    })??;
    let resource = parsed.with_resource_types(
        CatalogRecordSelection::RecordIndex(target),
        None,
        |snapshot| witness(RESOURCE_TYPES, snapshot.rows(), &catalog),
    )??;
    Ok([groups, members, parents, resource])
}

fn member_target(members: &[ResourceGroupMembers]) -> TestResult<u32> {
    let reference = members
        .iter()
        .filter_map(|row| row.resources().value())
        .flatten()
        .find_map(SourcedValue::value)
        .ok_or("the original resource membership reference")?;
    reference
        .target_record_index()
        .value()
        .copied()
        .ok_or_else(|| "the membership reference must have a physical ResourceType target".into())
}

fn witness<Row: JsonSchema + Serialize>(
    dataset: &'static str,
    rows: &[Row],
    catalog: &Value,
) -> TestResult<Route> {
    let expected = catalog
        .pointer(dataset)
        .and_then(|table| table.get("rows"))
        .ok_or_else(|| format!("missing canonical rows for {dataset}"))?;
    canonical_json::assert_canonical(rows, expected, dataset)?;
    Route::queried_rows(dataset, rows, REMAINING)
}
