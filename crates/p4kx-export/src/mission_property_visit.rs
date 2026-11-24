// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Record, SourceInventory, TreeBuilder,
};

use super::{
    MissionFlowConfiguration, MissionPropertyConfiguration, MissionPropertySource, OWNER_KINDS,
    availability, read_owner,
};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::unlocalized_record_query::plan::read_owner as read_record;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    ParsedCatalogQueries,
};
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::DatasetAvailability;
use crate::guid_text::CatalogGuid;
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_read::{RecordAdmission, classify};

/// One physical mission owner's original arena, property rows and flow rows.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct MissionPropertyOwnerSnapshot<'source> {
    /// Property rows in the owner's canonical declaration order.
    pub rows: Vec<MissionPropertyConfiguration>,
    /// Flow rows in the owner's canonical declaration order.
    pub flows: Vec<MissionFlowConfiguration>,
    /// The physical owner and the original arena referenced by both tables.
    pub source: MissionPropertySource<'source>,
    /// The original inputs and uses captured for this owner.
    pub source_inventory: SourceInventory,
    /// Whether the original fields and interpreted flow are readable.
    pub fields_complete: bool,
}

/// The terminal coverage of both original mission configuration tables.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct MissionPropertyVisitSummary {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability shared by both original configuration tables.
    pub availability: DatasetAvailability,
    /// The number of readable physical owners supplied.
    pub owner_count: usize,
    /// The number of property rows supplied.
    pub property_count: usize,
    /// The number of flow rows supplied.
    pub flow_count: usize,
    /// Original owner admission and read coverage.
    pub coverage: CatalogRecordCoverage,
}

/// Visits Template owners then Broker owners in canonical GUID order, preserving physical ties.
/// Stage consumer changes until the visit succeeds and its coverage meets the caller's requirements.
///
/// # Errors
///
/// Returns input, Archive-change, cancellation or visitor errors.
pub fn visit_mission_properties<E: From<CatalogError>>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    visit: impl for<'source> FnMut(&MissionPropertyOwnerSnapshot<'source>) -> Result<(), E>,
) -> Result<MissionPropertyVisitSummary, E> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .visit_mission_properties(selection, cancel, visit)
}

/// Visits original mission owners from a previously decoded Entry of the same Archive.
///
/// # Errors
///
/// Returns query or visitor errors, including a mismatch for a foreign Entry.
pub fn visit_mission_properties_from<E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    visit: impl for<'source> FnMut(&MissionPropertyOwnerSnapshot<'source>) -> Result<(), E>,
) -> Result<MissionPropertyVisitSummary, E> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .visit_mission_properties(selection, cancel, visit)
}

impl CatalogQuerySession<'_> {
    /// Visits the selected original mission configuration owners in this session.
    ///
    /// # Errors
    ///
    /// Returns query or visitor errors.
    pub fn visit_mission_properties<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl for<'source> FnMut(&MissionPropertyOwnerSnapshot<'source>) -> Result<(), E>,
    ) -> Result<MissionPropertyVisitSummary, E> {
        self.parsed(cancel)?
            .visit_mission_properties(selection, cancel, visit)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Visits selected original mission configuration owners in the parsed DataForge.
    ///
    /// # Errors
    ///
    /// Returns query or visitor errors.
    pub fn visit_mission_properties<E: From<CatalogError>>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        visit: impl for<'source> FnMut(&MissionPropertyOwnerSnapshot<'source>) -> Result<(), E>,
    ) -> Result<MissionPropertyVisitSummary, E> {
        self.project_fallible(cancel, |archive, entry, forge| {
            visit_owners(archive, entry, forge, selection, cancel, visit)
        })
    }
}

fn visit_owners<'source, E: From<CatalogError>>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &'source DataForge<'source>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    mut visit: impl FnMut(&MissionPropertyOwnerSnapshot<'source>) -> Result<(), E>,
) -> Result<MissionPropertyVisitSummary, E> {
    let mut summary = MissionPropertyVisitSummary {
        build: archive.build().cloned(),
        availability: availability(forge),
        owner_count: 0,
        property_count: 0,
        flow_count: 0,
        coverage: CatalogRecordCoverage::default(),
    };
    let records = select_owners(forge, selection, cancel, &mut summary.coverage)?;
    let mut trees = TreeBuilder::new();
    for OwnerHeader { family, record, .. } in records {
        checkpoint(cancel)?;
        let Some(owner) = read_record(&mut trees, &record, &mut summary.coverage)? else {
            continue;
        };
        let origins = CatalogOrigins::for_archive_records(archive, entry)?;
        let input = OriginalCatalogInput::new(forge, &origins);
        let scoped = read_owner(&input, family, &owner.record, &owner.tree, cancel)?;
        let snapshot = MissionPropertyOwnerSnapshot {
            rows: scoped.rows,
            flows: scoped.flows,
            source: scoped.source,
            source_inventory: origins.uses.finish_with(|| checkpoint(cancel))?,
            fields_complete: scoped.complete,
        };
        if !snapshot.fields_complete {
            summary.coverage.incomplete_fields.push(record.index());
        }
        checkpoint(cancel)?;
        visit(&snapshot)?;
        summary.owner_count += 1;
        summary.property_count += snapshot.rows.len();
        summary.flow_count += snapshot.flows.len();
    }
    Ok(summary)
}

struct OwnerHeader<'source> {
    family_index: usize,
    family: FlowOwnerKind,
    record: Record<'source>,
}

fn select_owners<'source>(
    forge: &'source DataForge<'source>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    coverage: &mut CatalogRecordCoverage,
) -> Result<Vec<OwnerHeader<'source>>, CatalogError> {
    let mut records = Vec::new();
    selection.for_each(forge, cancel, |record| {
        if let Some(header) = selected_header(record, coverage) {
            records.push(header);
        }
        Ok(())
    })?;
    records.sort_by_cached_key(|header| {
        (
            header.family_index,
            CatalogGuid(header.record.guid()).text(),
        )
    });
    Ok(records)
}

fn selected_header<'source>(
    record: &Record<'source>,
    coverage: &mut CatalogRecordCoverage,
) -> Option<OwnerHeader<'source>> {
    if record.record_path().is_ok_and(|path| {
        !OWNER_KINDS
            .iter()
            .any(|family| family.catalog_kind().records().matches_path(path))
    }) {
        return None;
    }
    let Ok(source_type) = record.type_name() else {
        coverage.unreadable_headers.push(record.index());
        return None;
    };
    let (family_index, family) = OWNER_KINDS
        .into_iter()
        .enumerate()
        .find(|(_, family)| family.catalog_kind().records().type_name == source_type)?;
    match classify(record, family.catalog_kind()) {
        Ok(RecordAdmission::Selected) => {
            return Some(OwnerHeader {
                family_index,
                family,
                record: *record,
            });
        }
        Ok(RecordAdmission::Leftover) => coverage.skipped_leftovers += 1,
        Ok(RecordAdmission::Other) => {}
        Err(_) => coverage.unreadable_headers.push(record.index()),
    }
    None
}
