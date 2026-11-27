// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, Value};

use super::super::Observed;
use super::playability_inputs::ShipPlayabilityInputs;
use super::selection::SelectedSource;
use crate::CatalogError;
use crate::source_fields::FieldError;

pub(super) fn selected<'reader, 'scope, 'read, 'source, T>(
    fields: Observed<'reader, 'scope, 'read, 'source>,
    selection: &SelectedSource<'_, 'source>,
    read: impl FnOnce(
        Observed<'reader, 'scope, 'read, 'source>,
    ) -> Result<SourcedValue<T>, CatalogError>,
) -> Result<SourcedValue<T>, CatalogError> {
    let Some(value) = selection.value else {
        return Ok(selection.missing(fields.originals.sources()));
    };
    let owner = fields.at_owner(fields.originals.fact(value)?);
    selection.bind(fields.originals.sources(), read(owner)?)
}

pub(super) fn text<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    name: &str,
    notes: &mut Vec<FieldError>,
) -> Result<SourcedValue<&'source str>, CatalogError> {
    fields.scalar(name, "a nullable String or Enum", notes, |fact| {
        Ok(match fact.value() {
            Value::String(value) | Value::Enum(value) => Some(*value),
            _ => None,
        })
    })
}

pub(super) fn crew<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    selected_source: &SelectedSource<'_, 'source>,
    notes: &mut Vec<FieldError>,
) -> Result<ShipPlayabilityInputs<'source>, CatalogError> {
    Ok(ShipPlayabilityInputs {
        crew_manifest_component_ordinal: selected_source.ordinal(fields.originals)?,
        crew_manifest: selected(fields, selected_source, |owner| {
            owner.record_link("crewManifest", notes)
        })?,
    })
}
