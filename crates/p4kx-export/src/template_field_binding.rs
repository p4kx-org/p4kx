// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::template_source_refs::FlowFieldBinding;

pub(crate) fn read<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    parent: &OriginalValueFact<'scope, 'read, 'source>,
    name: &str,
) -> Result<FlowFieldBinding, CatalogError> {
    match fields.field(Some(parent), name)? {
        Some((index, _)) => Ok(FlowFieldBinding::Field {
            field_index: DecimalU64::new(index),
        }),
        None => Ok(FlowFieldBinding::Absent {
            origin: fields.absent(Some(parent))?,
        }),
    }
}
