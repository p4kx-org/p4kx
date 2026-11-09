// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUseRole, SourceValue};

use super::contract::ResourceGroupParent;
use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;
use crate::resource_group::ParentChoice;

/// Materializes the exact first-parent result from retained original selection observations.
pub(crate) fn read(
    origins: &CatalogOrigins,
    record_index: u32,
    guid: CatalogGuid,
    choice: ParentChoice<'_>,
) -> Result<ResourceGroupParent, CatalogError> {
    let context = origins.record(record_index);
    let mut inputs = choice.inputs;
    inputs.push(context.observe_header(RecordHeaderField::Guid, SourceUseRole::Selector)?);
    let value = choice
        .target
        .map(|target| {
            let parent = origins.record(target.record_index);
            inputs.push(parent.observe_header(RecordHeaderField::Guid, SourceUseRole::Payload)?);
            Ok::<_, CatalogError>(CatalogRecordReference {
                target_record_index: parent.header_selector(
                    RecordHeaderField::RecordIndex,
                    SourceValue::Present(target.record_index),
                )?,
                guid: parent
                    .header_value(RecordHeaderField::Guid, SourceValue::Present(target.guid))?,
                record_path: parent.header_conversion(
                    RecordHeaderField::FileName,
                    SourceValue::Present(target.record_path.clone()),
                )?,
                target_type: parent.header_selector(
                    RecordHeaderField::SourceType,
                    SourceValue::Present(target.source_type.clone()),
                )?,
                status: DefinitionStatus::Present,
            })
        })
        .transpose()?;
    Ok(ResourceGroupParent {
        guid: context.header_value(RecordHeaderField::Guid, SourceValue::Present(guid))?,
        record_index: context.header_selector(
            RecordHeaderField::RecordIndex,
            SourceValue::Present(record_index),
        )?,
        parent: origins
            .uses
            .value(SourceValue::Present(value), &inputs, choice.complete),
    })
}
