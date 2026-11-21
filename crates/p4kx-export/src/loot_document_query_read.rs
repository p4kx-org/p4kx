// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, CancelToken, DataForge, DataForgeEntry, SourceUseRole, SourceValue, SourcedValue,
};

use super::{
    LootDocumentAdmission, LootDocumentCoverage, LootDocumentQuery, LootDocumentReachability,
    LootDocumentSelection, LootDocumentSnapshot,
};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::DatasetAvailability;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::EntryCapture;
use crate::export_entry_counts::ExportEntryCounts;
use crate::export_health_entries;
use crate::loot_entxml::documents::LootEntityContainerDocument;
use crate::loot_walk::{self, SelectedDocument};
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::starmap_containers::{PlacementScope, Placements};
use crate::starmap_index::Index;
use crate::{CatalogError, ExportAccountingError, Filter, Kind};

pub(super) fn run(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    query: LootDocumentQuery<'_>,
    cancel: Option<&CancelToken>,
) -> Result<LootDocumentSnapshot, CatalogError> {
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let original = OriginalCatalogInput::new(forge, &origins);
    let admission = admit(forge, &origins, query.catalog_filter, cancel)?;
    read(
        archive,
        query,
        admission,
        cancel,
        &EntryCapture::for_input(&original),
    )
}

struct Admission {
    status: LootDocumentAdmission,
    record: Option<SourcedValue<u32>>,
}

fn admit(
    forge: &DataForge<'_>,
    origins: &CatalogOrigins,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Admission, CatalogError> {
    let mut complete = true;
    for record in forge.records() {
        checkpoint(cancel)?;
        match record.record_path() {
            Ok(path) if PlacementScope::admits_loot_path(path, filter) => {
                let mut uses = Vec::new();
                let original = origins.record(record.index());
                original.append_header_input(
                    RecordHeaderInput::NormalizedPath,
                    SourceUseRole::Selector,
                    &mut uses,
                )?;
                original.append_header_input(
                    RecordHeaderInput::Field(RecordHeaderField::RecordIndex),
                    SourceUseRole::Payload,
                    &mut uses,
                )?;
                return Ok(Admission {
                    status: LootDocumentAdmission::Selected,
                    record: Some(origins.uses.value(
                        SourceValue::Present(record.index()),
                        &uses,
                        true,
                    )),
                });
            }
            Ok(_) => {}
            Err(_) => complete = false,
        }
    }
    let status = if complete {
        LootDocumentAdmission::Absent
    } else {
        LootDocumentAdmission::Unavailable
    };
    Ok(Admission {
        status,
        record: None,
    })
}

fn read(
    archive: &Archive,
    query: LootDocumentQuery<'_>,
    admission: Admission,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<LootDocumentSnapshot, CatalogError> {
    let directory_complete = archive.root_directory_entry_count().is_some()
        && crate::export_archive_health::inventory(archive).is_none();
    let mut coverage = LootDocumentCoverage {
        directory_complete,
        admission: admission.status,
        admission_record: admission.record,
        reachability: reachability(
            query.selection,
            false,
            admission.status == LootDocumentAdmission::Absent,
        ),
        placements_complete: false,
        documents_complete: admission.status == LootDocumentAdmission::Absent,
        visited_loot_containers: 0,
        placement_entries: ExportEntryCounts::default(),
        loot_entries: ExportEntryCounts::default(),
    };
    let rows = if coverage.admission == LootDocumentAdmission::Selected {
        read_selected(archive, query, cancel, capture, &mut coverage)?
    } else {
        Vec::new()
    };
    let availability = if !rows.is_empty() {
        DatasetAvailability::Available
    } else if coverage.documents_complete {
        DatasetAvailability::Absent
    } else {
        DatasetAvailability::Unavailable
    };
    checkpoint(cancel)?;
    let source_inventory = capture.source_uses().finish_with(|| checkpoint(cancel))?;
    Ok(LootDocumentSnapshot {
        build: archive.build().cloned(),
        selection: query.selection,
        availability,
        rows,
        coverage,
        source_inventory,
    })
}

fn read_selected(
    archive: &Archive,
    query: LootDocumentQuery<'_>,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    coverage: &mut LootDocumentCoverage,
) -> Result<Vec<LootEntityContainerDocument>, CatalogError> {
    let selected = match selection(archive, query.selection)? {
        Selection::All => None,
        Selection::Physical(document) => Some(document),
        Selection::Missing => {
            let observed_count = capture
                .source_uses()
                .observe_root_directory_entry_count(archive)?
                .is_some();
            coverage.documents_complete = observed_count && coverage.directory_complete;
            coverage.reachability =
                reachability(query.selection, false, coverage.documents_complete);
            return Ok(Vec::new());
        }
    };
    let placements = Placements::read_loot_seeds(
        archive,
        cancel,
        query.placement_budget,
        capture,
        PlacementScope::RecordSelection,
    )?;
    let routes_complete = !placements.placement_routes_incomplete
        && placements.counts.unreadable == 0
        && placements.counts.missing == placements.counts.verified_missing
        && placements.counts.malformed == 0
        && !placements
            .ledger
            .incomplete_inventories
            .contains(&Kind::StarmapObjects);
    coverage.placement_entries = entry_counts(placements.ledger.entries, Kind::StarmapObjects)?;
    coverage.placements_complete =
        coverage.directory_complete && routes_complete && successful(coverage.placement_entries);
    let index = Index::new(archive);
    let mut rows = Vec::new();
    let walked = loot_walk::visit_documents(
        archive,
        &index,
        &placements.opened,
        cancel,
        capture,
        selected,
        |entry, document| {
            rows.push(((entry, document.physical_index()?), document));
            Ok(())
        },
    )?;
    coverage.visited_loot_containers = walked.summaries.len();
    coverage.loot_entries = entry_counts(walked.ledger.entries, Kind::LootContainers)?;
    let exhausted =
        coverage.placements_complete && walked.routes_complete && successful(coverage.loot_entries);
    coverage.reachability = reachability(query.selection, walked.found_owner, exhausted);
    coverage.documents_complete = walked.documents_complete && (walked.found_owner || exhausted);
    rows.sort_unstable_by_key(|(key, _)| *key);
    Ok(rows.into_iter().map(|(_, row)| row).collect())
}

enum Selection {
    All,
    Physical(SelectedDocument),
    Missing,
}

fn selection(
    archive: &Archive,
    selection: LootDocumentSelection,
) -> Result<Selection, CatalogError> {
    let (outer, nested) = match selection {
        LootDocumentSelection::All => return Ok(Selection::All),
        LootDocumentSelection::Container {
            container_entry_index,
        } => (container_entry_index, None),
        LootDocumentSelection::Document {
            container_entry_index,
            document_entry_index,
        } => (container_entry_index, Some(document_entry_index)),
    };
    let outer = usize::try_from(outer).map_err(|_| ExportAccountingError::CounterOverflow)?;
    let document = nested
        .map(usize::try_from)
        .transpose()
        .map_err(|_| ExportAccountingError::CounterOverflow)?;
    let Some(entry) = archive.entries().get(outer) else {
        return Ok(Selection::Missing);
    };
    if entry.index() != outer {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    Ok(Selection::Physical(SelectedDocument {
        container_position: outer,
        entity_index: document,
    }))
}

const fn reachability(
    selection: LootDocumentSelection,
    reached: bool,
    exhausted: bool,
) -> LootDocumentReachability {
    if matches!(selection, LootDocumentSelection::All) {
        LootDocumentReachability::Enumeration
    } else if reached {
        LootDocumentReachability::Reached
    } else if exhausted {
        LootDocumentReachability::NotReached
    } else {
        LootDocumentReachability::Unavailable
    }
}

fn entry_counts(
    observed: Vec<crate::export_entry_ledger::ArchiveEntryObservation>,
    kind: Kind,
) -> Result<ExportEntryCounts, CatalogError> {
    let entries = export_health_entries::compile(observed)?;
    Ok(export_health_entries::counts(kind, &entries.sources)?)
}

const fn successful(entries: ExportEntryCounts) -> bool {
    entries.failed_required_reads == 0
        && entries.failed_projections == 0
        && entries.failed_dependencies == 0
        && entries.failed_selection_candidates == 0
}
