// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::sandbox_trigger::SandboxTrigger;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

pub(crate) const SOURCE_TYPE: &str = "SandboxTriggerRecord";

/// An authored Record with ordered manual reputation trigger declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SandboxTriggerRecordDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    trigger_manual_params: SourcedValue<Vec<SourcedValue<SandboxTrigger<'source>>>>,
    authored_fields: SourceArenaOutput<'source>,
}

pub(crate) struct PreparedSandboxTriggerRecord<'source> {
    definition: SandboxTriggerRecordDefinition<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_index: u32,
    pub(crate) physical_path: &'source str,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) struct SandboxTriggerRecordRead<'source> {
    pub(crate) value: PreparedSandboxTriggerRecord<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads one physical sandbox trigger Record.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<SandboxTriggerRecordRead<'source>, CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, input, &capture, &numbers);
    let mut notes = Vec::new();
    let trigger_manual_params = SandboxTrigger::read_record_members(fields, &mut notes)?;
    let authored_fields =
        fields.authored_members_except([("triggerManualParams", trigger_manual_params.state())])?;
    Ok(SandboxTriggerRecordRead {
        value: PreparedSandboxTriggerRecord {
            definition: SandboxTriggerRecordDefinition {
                source: CanonicalRecordHeader::read(tree, projection.origins())?,
                trigger_manual_params,
                authored_fields,
            },
            physical_guid: tree.guid(),
            physical_index: tree.record_index(),
            physical_path: tree.record_path(),
            numbers: numbers.into_inner(),
        },
        notes,
    })
}

/// Builds the sandbox trigger table with its physical source-family availability.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedSandboxTriggerRecord<'source>>,
) -> CatalogTable<SandboxTriggerRecordDefinition<'source>> {
    CatalogTable::new(
        forge,
        &[SOURCE_TYPE],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}
