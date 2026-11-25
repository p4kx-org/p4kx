// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge};

use super::{CatalogRecordCoverage, CatalogRecordSelection, Owner, Plan};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::CatalogTable;
use crate::catalog_template_modules::mission_owner_definition::{self, MissionOwnerDefinition};
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_read::{self, RecordAdmission};
use crate::{CatalogError, Kind};

pub(super) fn plan<'source>(
    forge: &'source DataForge<'source>,
    kind: Kind,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<Plan<'source>, CatalogError> {
    if matches!(selection, CatalogRecordSelection::RecordIndex(_)) {
        return Plan::read_rule(forge, kind.records(), selection, cancel, None);
    }
    Plan::read_with(forge, selection, cancel, None, |record, _, coverage| {
        Ok(match record_read::classify(record, kind) {
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
        })
    })
}

pub(super) fn read<'source, Row: From<MissionOwnerDefinition<'source>>>(
    forge: &DataForge<'_>,
    mut owners: Vec<Owner<'source>>,
    input: &OriginalCatalogInput<'_, 'source>,
    coverage: &mut CatalogRecordCoverage,
    family: FlowOwnerKind,
    cancel: Option<&CancelToken>,
) -> Result<CatalogTable<Row>, CatalogError> {
    sort_by_guid(&mut owners, |owner| CatalogGuid(owner.record.guid()));
    let mut rows = Vec::with_capacity(owners.len());
    for Owner { record, tree } in owners {
        checkpoint(cancel)?;
        let read = mission_owner_definition::read_selected(input, &record, &tree, family, cancel)?;
        if !read.selection_complete {
            coverage.incomplete_fields.push(record.index());
        }
        rows.push(Row::from(read.value));
    }
    let original = family.catalog_kind().records().type_name;
    Ok(CatalogTable::new(forge, &[original], rows))
}
