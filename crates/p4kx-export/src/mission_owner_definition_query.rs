// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge};

use super::{CatalogRecordCoverage, CatalogRecordSelection, OrderedOwnerPlan, rule_allows};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::unlocalized_record_query::plan::Owner;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_table::CatalogTable;
use crate::catalog_template_modules::mission_owner_definition::{self, MissionOwnerDefinition};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_read::{self, RecordAdmission};
use crate::{CatalogError, Kind};

pub(super) fn plan<'source>(
    forge: &'source DataForge<'source>,
    kind: Kind,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<OrderedOwnerPlan<'source>, CatalogError> {
    OrderedOwnerPlan::select(forge, selection, cancel, |record, coverage| {
        if matches!(selection, CatalogRecordSelection::RecordIndex(_)) {
            return rule_allows(record, kind.records(), coverage);
        }
        match record_read::classify(record, kind) {
            Ok(RecordAdmission::Selected) => true,
            Ok(RecordAdmission::Leftover) => {
                coverage.skipped_leftovers += 1;
                false
            }
            Ok(RecordAdmission::Other) => false,
            Err(_) => {
                coverage.unreadable_headers.push(record.index());
                false
            }
        }
    })
}

pub(super) fn read<'source, Row: From<MissionOwnerDefinition<'source>>>(
    forge: &DataForge<'_>,
    plan: OrderedOwnerPlan<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    family: FlowOwnerKind,
    cancel: Option<&CancelToken>,
) -> Result<(CatalogTable<Row>, CatalogRecordCoverage), CatalogError> {
    let (rows, coverage) = plan.project(cancel, |owner, coverage| {
        let read = read_owner(input, owner, coverage, family, cancel)?;
        input
            .origins()
            .uses
            .compact_record_uses(|| checkpoint(cancel))?;
        Ok(Row::from(read.value))
    })?;
    let original = family.catalog_kind().records().type_name;
    Ok((CatalogTable::new(forge, &[original], rows), coverage))
}

pub(super) fn read_owner<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    Owner { record, tree }: Owner<'source>,
    coverage: &mut CatalogRecordCoverage,
    family: FlowOwnerKind,
    cancel: Option<&CancelToken>,
) -> Result<ScopedSourceRead<MissionOwnerDefinition<'source>>, CatalogError> {
    let read = mission_owner_definition::read_selected(input, &record, &tree, family, cancel)?;
    if !read.selection_complete {
        coverage.incomplete_fields.push(record.index());
    }
    Ok(read)
}
