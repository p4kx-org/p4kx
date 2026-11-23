// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, CancelToken, DataForge, DataForgeEntry, Filter, Guid, Record, RecordTree, TreeBuilder,
};

use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::unlocalized_record_query::plan;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::{checkpoint, load_error};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::DatasetAvailability;
use crate::catalog_v3::starmap_object_definition::SOURCE_TYPE;
use crate::contract_starmap::ShopLinkHeader;
use crate::entity_selection::{self, Selected};
use crate::guid_text::CatalogGuid;
use crate::scan_health::SourceTrace;
use crate::source_header::RecordNaming;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) struct Survey {
    pub(crate) origins: CatalogOrigins,
    pub(crate) availability: DatasetAvailability,
    pub(crate) coverage: CatalogRecordCoverage,
}

#[derive(Clone, Copy)]
pub(crate) enum NameInput<'view, 'data> {
    Standalone,
    Decoded(&'view DataForgeEntry),
    Parsed(&'view DataForgeEntry, &'view DataForge<'data>),
}

impl NameInput<'_, '_> {
    fn visit(
        self,
        archive: &Archive,
        capture: &EntryCapture<'_>,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        emit: impl FnMut(u32, RecordNaming<'_>) -> Result<(), CatalogError>,
    ) -> Result<Survey, CatalogError> {
        match self {
            Self::Standalone => visit(archive, capture, selection, cancel, emit),
            Self::Decoded(entry) => {
                let forge = entry.open().map_err(CatalogError::DataForge)?;
                visit_parsed(archive, entry, &forge, capture, selection, cancel, emit)
            }
            Self::Parsed(entry, forge) => {
                visit_parsed(archive, entry, forge, capture, selection, cancel, emit)
            }
        }
    }
}

/// Visits admitted physical Starmap names and discards each tree before visiting the next.
pub(crate) fn visit(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    emit: impl FnMut(u32, RecordNaming<'_>) -> Result<(), CatalogError>,
) -> Result<Survey, CatalogError> {
    let entry =
        DataForgeEntry::read_until(archive, &|| cancel.is_some_and(CancelToken::is_cancelled))
            .map_err(load_error)?;
    let forge = entry.open().map_err(CatalogError::DataForge)?;
    visit_parsed(archive, &entry, &forge, capture, selection, cancel, emit)
}

fn visit_parsed(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    capture: &EntryCapture<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    mut emit: impl FnMut(u32, RecordNaming<'_>) -> Result<(), CatalogError>,
) -> Result<Survey, CatalogError> {
    let origins = CatalogOrigins::for_archive_records_with_uses(
        archive,
        entry,
        capture.source_uses().clone(),
    )?;
    let (availability, coverage) =
        visit_records(forge, selection, None, cancel, |record, tree| {
            emit(record.index(), RecordNaming::read(tree, &origins)?)
        })?;
    Ok(Survey {
        origins,
        availability,
        coverage,
    })
}

/// Visits one admitted physical Starmap tree at a time from the supplied DataForge.
pub(super) fn visit_records<'source>(
    forge: &'source DataForge<'source>,
    selection: CatalogRecordSelection,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
    mut emit: impl FnMut(&Record<'source>, &RecordTree<'source>) -> Result<(), CatalogError>,
) -> Result<(DatasetAvailability, CatalogRecordCoverage), CatalogError> {
    let mut trees = TreeBuilder::new();
    let mut coverage = CatalogRecordCoverage::default();
    let mut observed = false;
    selection.for_each(forge, cancel, |record| {
        checkpoint(cancel)?;
        if selected(record, filter, &mut coverage)
            && let Some(owner) = plan::read_owner(&mut trees, record, &mut coverage)?
        {
            emit(&owner.record, &owner.tree)?;
            observed = true;
        }
        Ok(())
    })?;
    checkpoint(cancel)?;
    let availability = if observed {
        DatasetAvailability::Available
    } else {
        DatasetAvailability::from_source_declarations(forge, |name| name == SOURCE_TYPE)
    };
    Ok((availability, coverage))
}

fn selected(
    record: &Record<'_>,
    filter: Option<&Filter>,
    coverage: &mut CatalogRecordCoverage,
) -> bool {
    match entity_selection::select(record, filter) {
        Err(_)
            if SourceTrace::new(record)
                .selection_candidate_kind()
                .is_none_or(|kind| kind == Kind::StarmapObjects) =>
        {
            coverage.unreadable_headers.push(record.index());
        }
        Ok(Some(Selected::Leftover(Kind::StarmapObjects))) => coverage.skipped_leftovers += 1,
        Ok(Some(Selected::Entity(Kind::StarmapObjects))) => return true,
        Err(_) | Ok(_) => {}
    }
    false
}

pub(crate) struct StarmapNameOwner {
    pub(crate) guid: CatalogGuid,
    record_index: u32,
    pub(crate) class_name: String,
    pub(crate) header: ShopLinkHeader,
}

/// Every observed physical name owner, with the descriptor demand for that survey.
pub(crate) struct AllStarmapNames {
    pub(crate) owners: Vec<StarmapNameOwner>,
    pub(crate) coverage: CatalogRecordCoverage,
    descriptor_targets: Vec<Guid>,
}

impl AllStarmapNames {
    pub(crate) fn read(
        archive: &Archive,
        capture: &EntryCapture<'_>,
        input: NameInput<'_, '_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut owners = Vec::new();
        let survey = input.visit(
            archive,
            capture,
            CatalogRecordSelection::All,
            cancel,
            |record_index, name| {
                let guid = *name
                    .guid
                    .value()
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                let class_name = name
                    .class_name
                    .value()
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                owners
                    .try_reserve(1)
                    .map_err(CatalogError::SourceUseAllocation)?;
                owners.push(StarmapNameOwner {
                    guid,
                    record_index,
                    class_name: (*class_name).to_owned(),
                    header: ShopLinkHeader {
                        guid: name.guid.origin().clone(),
                        class_name: name.class_name.origin().clone(),
                    },
                });
                Ok(())
            },
        )?;
        owners.sort_by_cached_key(|owner| (owner.guid.text(), owner.record_index));
        let mut descriptor_targets: Vec<_> = owners.iter().map(|owner| owner.guid.get()).collect();
        descriptor_targets.sort_unstable();
        descriptor_targets.dedup();
        Ok(Self {
            owners,
            coverage: survey.coverage,
            descriptor_targets,
        })
    }

    pub(crate) fn descriptor_targets(&self) -> &[Guid] {
        &self.descriptor_targets
    }

    pub(crate) const fn complete(&self) -> bool {
        self.coverage.unreadable_headers.is_empty() && self.coverage.unreadable_bodies.is_empty()
    }
}

#[cfg(test)]
#[path = "starmap_name_owner_scan_tests.rs"]
mod tests;
