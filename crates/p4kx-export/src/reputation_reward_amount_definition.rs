// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
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

pub(crate) const SOURCE_TYPE: &str = "SReputationRewardAmount";

/// A physical reputation reward amount with its exact signed value.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ReputationRewardAmountDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    editor_name: SourcedValue<&'source str>,
    reputation_amount: SourcedValue<i32>,
    authored_fields: SourceArenaOutput<'source>,
}

pub(crate) struct PreparedReputationRewardAmount<'source> {
    definition: ReputationRewardAmountDefinition<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_index: u32,
    pub(crate) physical_path: &'source str,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) struct ReputationRewardAmountRead<'source> {
    pub(crate) value: PreparedReputationRewardAmount<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads one physical reputation reward amount Record.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ReputationRewardAmountRead<'source>, CatalogError> {
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
    let editor_name = fields.borrowed_string("editorName", &mut notes)?;
    let reputation_amount = fields.int32("reputationAmount", &mut notes)?;
    let authored_fields = fields.authored_members_except([
        ("editorName", editor_name.state()),
        ("reputationAmount", reputation_amount.state()),
    ])?;
    Ok(ReputationRewardAmountRead {
        value: PreparedReputationRewardAmount {
            definition: ReputationRewardAmountDefinition {
                source: CanonicalRecordHeader::read(tree, projection.origins())?,
                editor_name,
                reputation_amount,
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

/// Builds the reputation reward amount table with its physical source-family availability.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedReputationRewardAmount<'source>>,
) -> CatalogTable<ReputationRewardAmountDefinition<'source>> {
    CatalogTable::new(
        forge,
        &[SOURCE_TYPE],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}
