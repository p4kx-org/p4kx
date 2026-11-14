// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};
use schemars::JsonSchema;
use serde::Serialize;

use super::ammo_presentation::AmmoPresentation;
use super::authored_ammo_inputs::AuthoredAmmoInputs;
use super::authored_fields::Observed;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::record_observation::RecordObservation;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError};

pub(crate) const SOURCE_TYPE: &str = "AmmoParams";

/// One physical ammunition definition and its original configuration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AmmoDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    #[serde(flatten)]
    inputs: AuthoredAmmoInputs<'source>,
    #[serde(flatten)]
    presentation: AmmoPresentation<'source>,
}

pub(crate) struct PreparedAmmoDefinition<'source> {
    definition: AmmoDefinition<'source>,
    pub(crate) physical_guid: Guid,
    pub(crate) physical_index: u32,
    pub(crate) physical_path: &'source str,
    pub(crate) numbers: Vec<CapturedNumber>,
}

/// Projects typed inputs from the same admitted tree used by the legacy view.
pub(crate) fn read<'source>(
    projection: &CatalogProjection<'_, 'source>,
    tree: &RecordTree<'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(PreparedAmmoDefinition<'source>, Vec<FieldError>), CatalogError> {
    let input = projection.original();
    let scope = input.record_scope(admission.record(), tree)?;
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let originals = OriginalFlowFields::read(&scope, cancel)?;
    let capture = TemplateFieldCapture::new(&originals, input, admission)?;
    let numbers = RefCell::new(Vec::new());
    let mut notes = Vec::new();
    let observed = Observed::definition(&originals, input, &capture, &numbers);
    let inputs = AuthoredAmmoInputs::read(observed, &mut notes)?;
    let presentation = AmmoPresentation::read(observed, projection, &mut notes)?;
    Ok((
        PreparedAmmoDefinition {
            definition: AmmoDefinition {
                source: CanonicalRecordHeader::read(tree, input.origins())?,
                inputs,
                presentation,
            },
            physical_guid: tree.guid(),
            physical_index: tree.record_index(),
            physical_path: tree.record_path(),
            numbers: numbers.into_inner(),
        },
        notes,
    ))
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    rows: Vec<PreparedAmmoDefinition<'source>>,
) -> CatalogTable<AmmoDefinition<'source>> {
    CatalogTable::new(
        forge,
        &[SOURCE_TYPE],
        rows.into_iter().map(|row| row.definition).collect(),
    )
}
