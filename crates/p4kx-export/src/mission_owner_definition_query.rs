// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge};

use super::{CatalogRecordCoverage, Owner};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_query_session::checkpoint;
use crate::catalog_table::CatalogTable;
use crate::catalog_template_modules::mission_owner_definition::{self, MissionOwnerDefinition};
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::mission_flow_owner::FlowOwnerKind;

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
