// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry, RecordTree};

use super::EquipmentSnapshot;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_queries::unlocalized_record_query::plan::Plan;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;

pub(super) type Project<'source, Row> = fn(
    &RecordTree<'source>,
    &OriginalCatalogInput<'_, 'source>,
    &RecordObservation<'_, 'source>,
    Option<&CancelToken>,
) -> Result<ScopedSourceRead<Row>, CatalogError>;

pub(super) fn with_parsed<'source, Row, T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &'source DataForge<'source>,
    selection: CatalogRecordSelection,
    (source_type, project): (&str, Project<'source, Row>),
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(EquipmentSnapshot<'source, Row>) -> T,
) -> Result<T, CatalogError> {
    let mut plan = Plan::read_type(forge, source_type, selection, cancel)?;
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut rows = Vec::with_capacity(plan.owners.len());
    for owner in plan.owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let read = project(&owner.tree, &input, &admission, cancel)?;
        if !read.selection_complete {
            plan.coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(read.value);
    }
    let table = CatalogTable::new(forge, &[source_type], rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(EquipmentSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        public_names: Vec::new(),
        game_texts: Vec::new(),
        requested_languages: Vec::new(),
        available_languages: Vec::new(),
        localization_notes: Vec::new(),
        source_inventory,
        coverage: plan.coverage,
        dependency_coverage: (),
    }))
}
