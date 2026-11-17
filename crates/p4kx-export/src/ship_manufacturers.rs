// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourcedValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::{Observed, missing_component};
use crate::catalog_read::original::ComponentSelection;
use crate::catalog_v3::authored_fields::authored_manufacturer::{
    AuthoredManufacturer, ManufacturerValue,
};
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError};

/// The authored manufacturer on the selected vehicle component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShipVehicleManufacturer<'source> {
    component_ordinal: SourcedValue<u32>,
    manufacturer: SourcedValue<ManufacturerValue<'source>>,
}

/// The authored `AttachDef` manufacturer of the selected ship attachment component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShipAttachmentManufacturer<'source> {
    component_ordinal: SourcedValue<u32>,
    attach_def_present: SourcedValue<bool>,
    manufacturer: SourcedValue<ManufacturerValue<'source>>,
}

pub(super) fn vehicle<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    selected: Option<&Value<'source>>,
    selection: &ComponentSelection,
    notes: &mut Vec<FieldError>,
) -> Result<ShipVehicleManufacturer<'source>, CatalogError> {
    let Some(selected) = selected else {
        return Ok(ShipVehicleManufacturer {
            component_ordinal: missing_component(fields.originals, selection),
            manufacturer: missing_component(fields.originals, selection),
        });
    };
    let source = fields.at_owner(fields.originals.fact(selected)?);
    let uses = source.originals.sources();
    let placement = uses.origin(&selection.uses, selection.complete);
    let manufacturer = AuthoredManufacturer::read(source, "manufacturer", notes)?.into_value();
    Ok(ShipVehicleManufacturer {
        component_ordinal: uses.add_value_origins(source.array_ordinal()?, &[&placement])?,
        manufacturer: uses.add_value_origins(manufacturer, &[&placement])?,
    })
}

pub(super) fn attachment<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    selected: Option<&Value<'source>>,
    notes: &mut Vec<FieldError>,
) -> Result<ShipAttachmentManufacturer<'source>, CatalogError> {
    let selection = fields.originals.component_selection(selected)?;
    let Some(selected) = selected else {
        return Ok(ShipAttachmentManufacturer {
            component_ordinal: missing_component(fields.originals, &selection),
            attach_def_present: missing_component(fields.originals, &selection),
            manufacturer: missing_component(fields.originals, &selection),
        });
    };
    let source = fields.at_owner(fields.originals.fact(selected)?);
    let uses = source.originals.sources();
    let placement = uses.origin(&selection.uses, selection.complete);
    let mut manufacturer = None;
    let decode = |definition, notes: &mut Vec<FieldError>| {
        manufacturer =
            Some(AuthoredManufacturer::read(definition, "Manufacturer", notes)?.into_value());
        Ok(true)
    };
    let attach_def_present = if source.storage_type("AttachDef")? == Some(DataType::StrongPointer) {
        source.declared_pointer("AttachDef", notes, decode)?
    } else {
        source.declared_class("AttachDef", notes, decode)?
    };
    let attach_def_present = uses.add_value_origins(attach_def_present, &[&placement])?;
    let manufacturer = match manufacturer {
        Some(cell) => uses.add_value_origins(cell, &[attach_def_present.origin()])?,
        None => attach_def_present.clone().try_map(|_| {
            Err::<_, CatalogError>(ExportAccountingError::ConflictingSourceDatum.into())
        })?,
    };
    Ok(ShipAttachmentManufacturer {
        component_ordinal: uses.add_value_origins(source.array_ordinal()?, &[&placement])?,
        attach_def_present,
        manufacturer,
    })
}
