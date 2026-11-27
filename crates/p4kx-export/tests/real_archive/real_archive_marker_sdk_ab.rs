// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::Instant;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries, ProvenanceMode,
};
use p4kx_extraction::{Archive, Guid};
use serde::Serialize;
use serde_json::{Value, json};

use crate::real_archive_catalog_table_parity::TestResult;
use crate::real_archive_scoped_equipment_sdk::{component_family_parity, equipment_input};

#[path = "sdk_marker_ab_origins.rs"]
mod origins;

#[path = "sdk_marker_ab_rows.rs"]
mod rows;
pub(crate) use rows::{Evidence, Rows, finish_lower};

const BUILD: &str = "13220821";
const OWNER_INDEX: u32 = 71;
const OWNER_GUID: &str = "99e0ea17-45fc-4700-8607-ab879d05a284";
const TAG_PATHS: usize = 18_961;
const MODE: &str = "P4KX_MARKER_SDK_MODE";
const SCOPE: &str = "P4KX_MARKER_SDK_SCOPE";
const WORKLOAD: &str = "P4KX_MARKER_SDK_WORKLOAD";
const RECEIPT: &str = "P4KX_MARKER_SDK_PROBE_RECEIPT";

#[test]
#[ignore = "installed Archive: configure the exact Build, mode, scope and private receipt"]
fn installed_marker_sdk_cold_query() -> TestResult {
    let mode = ProbeMode::parse(&env::var(MODE)?)?;
    let scope = Scope::parse(&env::var(SCOPE)?)?;
    let workload = Workload::parse(&env::var(WORKLOAD)?)?;
    let path = PathBuf::from(env::var_os(RECEIPT).ok_or("configure a private SDK probe receipt")?);
    let mut receipt = None;
    equipment_input::with_archive(|archive| {
        receipt = Some(probe(archive, mode, scope, workload)?);
        Ok(())
    })?;
    write_receipt(
        &path,
        &receipt.ok_or("the SDK probe produced no terminal result")?,
    )
}

pub(crate) fn write_receipt(path: &Path, receipt: &impl Serialize) -> TestResult {
    let parent = path.parent().ok_or("the private receipt needs a parent")?;
    if !path.is_absolute() || parent.canonicalize()? != env::temp_dir().canonicalize()? {
        return Err(
            "the SDK probe receipt must be in the configured private scratch directory".into(),
        );
    }
    let file = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer(file, receipt).map_err(|_| "write the technical SDK probe receipt".into())
}

fn probe(
    archive: &Archive,
    mode: ProbeMode,
    scope: Scope,
    workload: Workload,
) -> TestResult<Receipt> {
    if archive
        .build()
        .is_none_or(|build| build.change_number() != BUILD)
    {
        return Err("the SDK probe requires its exact independently verified LIVE Build".into());
    }
    let mut rows = Rows::new(mode, workload)?;
    let clock = Instant::now();
    let terminal = {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?
                .with_provenance_mode(mode.sdk());
        let parsed = session.parsed(None)?;
        match scope {
            Scope::LoadoutOwner => loadout(archive, &parsed, &mut rows)?,
            Scope::TagPaths => tag_paths(archive, &parsed, &mut rows)?,
        }
    };
    if rows.count() != scope.row_count() {
        return Err("the SDK probe did not hash the independently expected number of rows".into());
    }
    let evidence = rows.finish(&terminal)?;
    Ok(Receipt {
        protocol_version: 1,
        mode: mode.name(),
        inventory_mode: mode.name(),
        scope: scope.name(),
        workload: workload.name(),
        language_policy: "selected-empty",
        archive_build: BUILD,
        terminal_coverage_accepted: true,
        query_elapsed_milliseconds: clock.elapsed().as_secs_f64() * 1000.0,
        evidence,
        terminal,
    })
}

fn loadout(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    rows: &mut Rows,
) -> TestResult<Value> {
    let guid: Guid = OWNER_GUID.parse()?;
    let summary = parsed.visit_loadouts(
        CatalogRecordSelection::RecordIndex(OWNER_INDEX),
        None,
        |owner| -> TestResult {
            if owner.record_index != OWNER_INDEX
                || owner.entity_guid != guid
                || !owner.dependencies.is_complete()
            {
                return Err(
                    "unexpected Loadout physical owner or incomplete resolver dependencies".into(),
                );
            }
            let row = owner
                .row
                .as_ref()
                .ok_or("the selected Loadout owner did not emit a row")?;
            if row.record_index() != OWNER_INDEX
                || row.entity_guid() != guid
                || row.entity_kind() != owner.entity_kind
                || !row.issues().is_empty()
                || row.ports().len() != 1
            {
                return Err(
                    "the original Loadout row differs from its independent physical witness".into(),
                );
            }
            rows.observe_inventory(&owner.source_inventory)?;
            origins::loadout(rows, row);
            rows.observe_row(row)
        },
    )?;
    component_family_parity::require_complete(
        "/loadouts",
        summary.availability,
        summary.row_count,
        &summary.coverage,
    )?;
    if summary.build.as_ref() != archive.build()
        || summary.owner_count != 1
        || summary.row_count != 1
        || summary.coverage.skipped_leftovers != 0
        || !summary.dependencies_complete
    {
        return Err("the terminal Loadout summary differs from the independent witness".into());
    }
    Ok(json!({
        "availability": summary.availability,
        "owners": summary.owner_count,
        "rows": summary.row_count,
        "issueRows": 0,
        "skipped": summary.coverage.skipped_leftovers,
        "recordIndex": OWNER_INDEX,
        "ownerGuid": OWNER_GUID,
        "rootPortCount": 1,
        "dependenciesComplete": summary.dependencies_complete,
        "unreadableHeaders": summary.coverage.unreadable_headers,
        "unreadableBodies": summary.coverage.unreadable_bodies,
        "incompleteFields": summary.coverage.incomplete_fields,
    }))
}

fn tag_paths(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    rows: &mut Rows,
) -> TestResult<Value> {
    parsed.with_tag_paths(
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult<Value> {
            if snapshot.build.as_ref() != archive.build()
                || snapshot.rows.len() != TAG_PATHS
                || !snapshot.graph_inventory_complete()
                || !matches!(snapshot.availability, DatasetAvailability::Available)
            {
                return Err(
                    "the typed Tag path table differs from the independent installed census".into(),
                );
            }
            rows.observe_inventory(&snapshot.source_inventory)?;
            for tag in &snapshot.rows {
                let row = tag.fields()?;
                origins::tag_path(rows, &row);
                rows.observe_row(&row)?;
            }
            let graph = snapshot.graph_issues;
            Ok(json!({
                "availability": snapshot.availability,
                "rows": snapshot.rows.len(),
                "graphInventoryComplete": snapshot.graph_inventory_complete(),
                "graphUnclassified": graph.unclassified(),
                "graphUnreadable": graph.unreadable(),
                "graphDangling": graph.dangling(),
                "graphRepeated": graph.repeated(),
                "graphDuplicate": graph.duplicate(),
                "graphUnlisted": graph.unlisted(),
            }))
        },
    )?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    protocol_version: u32,
    mode: &'static str,
    inventory_mode: &'static str,
    scope: &'static str,
    workload: &'static str,
    language_policy: &'static str,
    archive_build: &'static str,
    terminal_coverage_accepted: bool,
    query_elapsed_milliseconds: f64,
    #[serde(flatten)]
    evidence: Evidence,
    #[serde(flatten)]
    terminal: Value,
}

#[derive(Clone, Copy)]
pub(crate) enum ProbeMode {
    Full,
    MarkersOnly,
}

impl ProbeMode {
    fn parse(name: &str) -> TestResult<Self> {
        match name {
            "full" => Ok(Self::Full),
            "markers-only" => Ok(Self::MarkersOnly),
            _ => Err("configure full or markers-only for the SDK probe".into()),
        }
    }

    pub(crate) const fn sdk(self) -> ProvenanceMode {
        match self {
            Self::Full => ProvenanceMode::Full,
            Self::MarkersOnly => ProvenanceMode::MarkersOnly,
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::MarkersOnly => "markers-only",
        }
    }
}

#[derive(Clone, Copy)]
enum Scope {
    LoadoutOwner,
    TagPaths,
}

impl Scope {
    fn parse(name: &str) -> TestResult<Self> {
        match name {
            "loadout-owner" => Ok(Self::LoadoutOwner),
            "tag-paths" => Ok(Self::TagPaths),
            _ => Err("configure loadout-owner or tag-paths for the SDK probe".into()),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::LoadoutOwner => "loadout-owner",
            Self::TagPaths => "tag-paths",
        }
    }

    const fn row_count(self) -> usize {
        match self {
            Self::LoadoutOwner => 1,
            Self::TagPaths => TAG_PATHS,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Workload {
    Verification,
    CountedSnapshot,
}

impl Workload {
    fn parse(name: &str) -> TestResult<Self> {
        match name {
            "query-and-verification" => Ok(Self::Verification),
            "query-and-counted-snapshot" => Ok(Self::CountedSnapshot),
            _ => Err("configure the verification or counted-snapshot SDK workload".into()),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Verification => "query-and-verification",
            Self::CountedSnapshot => "query-and-counted-snapshot",
        }
    }
}
