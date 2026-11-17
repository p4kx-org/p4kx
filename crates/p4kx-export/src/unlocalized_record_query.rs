// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, CancelToken, DataForge, DataForgeEntry, Record, RecordTree, SourceInventory,
    TreeBuilder,
};

use super::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::{CatalogTable, DatasetAvailability};
use crate::entity_selection::{self, Selected};
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::{CatalogError, Kind};

pub(super) struct RecordRows<Row> {
    pub(super) rows: Vec<Row>,
    pub(super) availability: DatasetAvailability,
    pub(super) source_inventory: SourceInventory,
    pub(super) coverage: CatalogRecordCoverage,
}

/// Projects one original DataForge family with the ordinary Catalog selection policy.
pub(super) fn read<Row>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    kind: Kind,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    mut project: impl for<'source> FnMut(
        &RecordTree<'source>,
        &OriginalCatalogInput<'source, 'source>,
        &RecordObservation<'_, 'source>,
    ) -> Result<(Row, Vec<FieldError>), CatalogError>,
) -> Result<RecordRows<Row>, CatalogError> {
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut coverage = CatalogRecordCoverage::default();
    let mut trees = TreeBuilder::new();
    let mut rows = Vec::new();
    selection.for_each(forge, cancel, |record| {
        if !selected(record, kind, &mut coverage) {
            return Ok(());
        }
        checkpoint(cancel)?;
        let Ok(tree) = trees.build(record) else {
            coverage.unreadable_bodies.push(record.index());
            return Ok(());
        };
        let admission = RecordObservation::capture(&input, record)?;
        let (row, notes) = project(&tree, &input, &admission)?;
        if notes
            .iter()
            .any(|note| note.description().issue.blocks_projection())
        {
            coverage.incomplete_fields.push(record.index());
        }
        rows.push(RowWithIdentity {
            guid: CatalogGuid(tree.guid()),
            row,
        });
        Ok(())
    })?;
    sort_by_guid(&mut rows, |row| row.guid);
    let table = CatalogTable::new(
        forge,
        &[kind.records().type_name],
        rows.into_iter().map(|row| row.row).collect(),
    );
    Ok(RecordRows {
        rows: table.rows,
        availability: table.availability,
        source_inventory: origins.uses.finish_with(|| checkpoint(cancel))?,
        coverage,
    })
}

struct RowWithIdentity<Row> {
    guid: CatalogGuid,
    row: Row,
}

fn selected(record: &Record<'_>, kind: Kind, coverage: &mut CatalogRecordCoverage) -> bool {
    let classified = record.type_name().and_then(|source_type| {
        if source_type == kind.records().type_name {
            entity_selection::select(record, None)
        } else {
            Ok(None)
        }
    });
    match classified {
        Err(_) => coverage.unreadable_headers.push(record.index()),
        Ok(Some(Selected::Leftover(actual))) if actual == kind => coverage.skipped_leftovers += 1,
        Ok(Some(Selected::Entity(actual))) if actual == kind => return true,
        Ok(_) => {}
    }
    false
}
