// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::location_vocabulary::{CLASS, FUNCTION, SIZE};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;
use crate::template_field_capture::TemplateFieldCapture;

type Reference<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;

/// The original dictionary links of one mission location.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationClassification<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "locationClass")]
    class: Reference<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "locationFunction")]
    function: Reference<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "locationSize")]
    size: Reference<'source>,
}

pub(crate) fn read<'source>(
    originals: &OriginalFlowFields<'_, '_, 'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    capture: &TemplateFieldCapture<'_, '_, '_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<LocationClassification<'source>>, CatalogError> {
    let numbers = RefCell::default();
    let fields = Observed::definition(originals, input, capture, &numbers);
    fields.declared_class_to(
        "locationData",
        "MissionLocationData",
        notes,
        |fields, notes| {
            Ok(LocationClassification {
                class: fields.record_link_to("locationClass", CLASS, notes)?,
                function: fields.record_link_to("locationFunction", FUNCTION, notes)?,
                size: fields.record_link_to("locationSize", SIZE, notes)?,
            })
        },
    )
}
