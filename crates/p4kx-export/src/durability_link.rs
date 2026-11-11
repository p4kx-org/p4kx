// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceUseRole, SourceUses, SourceValue, Value, ValueError};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{OriginalErrorParameter, OriginalValueFact, OriginalValuePart};
use crate::catalog_record_reference::CatalogRecordReference;
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::{
    SourcePayload, SourceRecordReference, SourceWeakReference, read_reference_payload,
};
use crate::{CatalogError, ExportAccountingError};

/// An authored lifecycle link with its actual Record or weak-pointer shape.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum HealthLink<'source> {
    RecordReference {
        #[schemars(with = "CatalogRecordReference")]
        value: Box<SourceRecordReference<'source>>,
    },
    WeakReference {
        value: Box<SourceWeakReference<'source>>,
    },
}

pub(super) fn read<'source>(
    fact: &OriginalValueFact<'_, '_, 'source>,
    sources: &SourceUses,
) -> Result<Option<(OriginalValuePart, HealthLink<'source>)>, CatalogError> {
    let result = match fact.value() {
        Value::Reference(_) => (OriginalValuePart::Value, resolved(fact, sources)?),
        Value::Error(ValueError::DanglingReference { guid }) => (
            OriginalValuePart::Error(OriginalErrorParameter::Guid),
            missing(
                fact,
                sources,
                CatalogGuid(*guid),
                DefinitionStatus::Dangling,
            )?,
        ),
        Value::Error(ValueError::UnexpectedReferenceType { guid, .. }) => (
            OriginalValuePart::Error(OriginalErrorParameter::Guid),
            missing(
                fact,
                sources,
                CatalogGuid(*guid),
                DefinitionStatus::UnexpectedTargetType,
            )?,
        ),
        _ => return Ok(None),
    };
    Ok(Some(result))
}

fn resolved<'source>(
    fact: &OriginalValueFact<'_, '_, 'source>,
    sources: &SourceUses,
) -> Result<HealthLink<'source>, CatalogError> {
    Ok(
        match read_reference_payload(fact.reference_observation()?, sources)? {
            SourcePayload::RecordReference { value } => HealthLink::RecordReference {
                value: Box::new(value),
            },
            SourcePayload::WeakReference { value } => HealthLink::WeakReference {
                value: Box::new(value),
            },
            _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
        },
    )
}

fn missing<'source>(
    fact: &OriginalValueFact<'_, '_, 'source>,
    sources: &SourceUses,
    guid: CatalogGuid,
    status: DefinitionStatus,
) -> Result<HealthLink<'source>, CatalogError> {
    if fact.field().storage().data_type() != DataType::Reference {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let part = OriginalValuePart::Error(OriginalErrorParameter::Guid);
    let selector = fact.observe(part, SourceUseRole::Selector)?;
    let negative = &[selector];
    Ok(HealthLink::RecordReference {
        value: Box::new(SourceRecordReference {
            target_record_index: sources.value(SourceValue::Absent, negative, true),
            guid: fact
                .bind(
                    part,
                    SourceValue::Present(guid),
                    SourceUseRole::Payload,
                    true,
                )?
                .into_cell(sources),
            record_path: sources.value(SourceValue::Absent, negative, true),
            target_type: sources.value(SourceValue::Absent, negative, true),
            status,
        }),
    })
}
