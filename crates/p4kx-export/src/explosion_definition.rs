// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_explosion::ExplosionParameters;
use super::authored_fields::Observed;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::CatalogFields;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

pub(crate) const SOURCE_TYPE: &str = "ExplosionRecord";
const PARAMETERS: &str = "explosionParams";

/// One independent physical definition and its authored `ExplosionParams` class.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ExplosionDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    explosion_params: SourcedValue<ExplosionParameters<'source>>,
}

/// One emitted definition with actual accounting identities and numeric receipts.
pub(crate) struct PreparedExplosionDefinition<'source> {
    definition: ExplosionDefinition<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_index: u32,
    pub(crate) physical_path: &'source str,
    pub(crate) numbers: Vec<CapturedNumber>,
}

pub(crate) struct ExplosionDefinitionRead<'source> {
    pub(crate) value: PreparedExplosionDefinition<'source>,
    pub(crate) notes: Vec<FieldError>,
}

/// Reads one selected Record without following links into foreign definitions.
pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<ExplosionDefinitionRead<'source>, CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let context = projection
        .origins()
        .record(tree.record_index())
        .with_cancel(cancel);
    let mut notes = Vec::new();
    CatalogFields::observed(tree.root(), "", &context, &capture)?
        .check_observed(&[PARAMETERS], &mut notes)?;
    let numbers = RefCell::new(Vec::new());
    let fields = Observed::definition(&originals, &capture, &numbers);
    let explosion_params = fields.inline_class(
        PARAMETERS,
        &["ExplosionParams"],
        &mut notes,
        ExplosionParameters::read,
    )?;
    Ok(ExplosionDefinitionRead {
        value: PreparedExplosionDefinition {
            definition: ExplosionDefinition {
                source: CanonicalRecordHeader::read(tree, projection.origins())?,
                explosion_params,
            },
            physical_guid: tree.guid(),
            physical_index: tree.record_index(),
            physical_path: tree.record_path(),
            numbers: numbers.into_inner(),
        },
        notes,
    })
}

/// Builds the definition table in canonical GUID and physical Record order.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedExplosionDefinition<'source>>,
) -> CatalogTable<ExplosionDefinition<'source>> {
    CatalogTable::new(
        forge,
        &[SOURCE_TYPE],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}
