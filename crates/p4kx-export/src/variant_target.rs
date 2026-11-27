// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, Guid, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::build::original_input::OriginalCatalogInput;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::variants::VariantMember;
use crate::{CatalogError, ExportAccountingError, schema_fields};

/// The physical Record chosen as the base of a variant.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = schema_fields::every_field_required)]
pub struct VariantTarget {
    target_record_index: SourcedValue<u32>,
    guid: SourcedValue<CatalogGuid>,
}

impl VariantTarget {
    /// Borrows the physical DataForge index of the chosen base.
    #[must_use]
    pub const fn target_record_index(&self) -> &SourcedValue<u32> {
        &self.target_record_index
    }

    /// Returns the original GUID of the chosen base.
    #[must_use]
    pub fn guid(&self) -> SourcedValue<Guid> {
        self.guid.map_ref(|guid| guid.0)
    }

    fn read(
        input: &OriginalCatalogInput<'_, '_>,
        member: VariantMember,
    ) -> Result<Self, CatalogError> {
        let record = input
            .forge()
            .record(member.record_index)
            .filter(|record| record.guid() == member.guid.0)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let context = input.record_context(&record)?;
        Ok(Self {
            target_record_index: context.header_selector(
                RecordHeaderField::RecordIndex,
                SourceValue::Present(record.index()),
            )?,
            guid: context
                .header_value(RecordHeaderField::Guid, SourceValue::Present(member.guid))?,
        })
    }
}

pub(crate) fn bind(
    input: &OriginalCatalogInput<'_, '_>,
    choice: Option<VariantMember>,
    origin: &FieldOrigin,
) -> Result<SourcedValue<VariantTarget>, CatalogError> {
    let state = if !origin.source_set_complete() {
        SourceValue::Unreadable
    } else if let Some(member) = choice {
        SourceValue::Present(VariantTarget::read(input, member)?)
    } else {
        SourceValue::Absent
    };
    let uses = &input.origins().uses;
    if let SourceValue::Present(target) = state {
        let index = target.target_record_index.origin().clone();
        let guid = target.guid.origin().clone();
        return Ok(uses.value_with_origins(SourceValue::Present(target), &[origin, &index, &guid])?);
    }
    Ok(uses.value_with_origins(state, &[origin])?)
}
