// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::{Observed, missing_component};
use crate::catalog_read::original::OriginalArraySelection;
use crate::catalog_v3::authored_fields::authored_manufacturer::{
    AuthoredManufacturer, ManufacturerValue,
};
use crate::resource_number::ResourceNumber;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

use crate::{CatalogError, ExportAccountingError};

/// The authored manufacturer on the selected vehicle component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ShipVehicleManufacturer<'source> {
    component_ordinal: SourcedValue<u32>,
    manufacturer: SourcedValue<ManufacturerValue<'source>>,
}

/// The authored `AttachDef` manufacturer of the selected ship attachment component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ShipAttachmentManufacturer<'source> {
    component_ordinal: SourcedValue<u32>,
    attach_def_present: SourcedValue<bool>,
    manufacturer: SourcedValue<ManufacturerValue<'source>>,
}

pub(super) fn vehicle<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    selected: Option<&Value<'source>>,
    selection: &OriginalArraySelection,
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

pub(super) struct AttachmentRead<'source> {
    pub(super) manufacturer: ShipAttachmentManufacturer<'source>,
    pub(super) size: SourcedValue<ResourceNumber>,
}

pub(super) fn attachment<'source>(
    fields: Observed<'_, '_, '_, 'source>,
    selected: Option<&Value<'source>>,
    selection: &OriginalArraySelection,
    notes: &mut Vec<FieldError>,
) -> Result<AttachmentRead<'source>, CatalogError> {
    let Some(selected) = selected else {
        return Ok(AttachmentRead {
            manufacturer: ShipAttachmentManufacturer {
                component_ordinal: missing_component(fields.originals, selection),
                attach_def_present: missing_component(fields.originals, selection),
                manufacturer: missing_component(fields.originals, selection),
            },
            size: missing_component(fields.originals, selection),
        });
    };
    let source = fields.at_owner(fields.originals.fact(selected)?);
    let uses = source.originals.sources();
    let placement = uses.origin(&selection.uses, selection.complete);
    let mut original = None;
    let attach_def_present = source.declared_owned("AttachDef", notes, |definition, notes| {
        original = Some((
            AuthoredManufacturer::read(definition, "Manufacturer", notes)?.into_value(),
            definition.authored_number("Size", notes)?,
        ));
        Ok(true)
    })?;
    let attach_def_present = uses.add_value_origins(attach_def_present, &[&placement])?;
    let (manufacturer, size) = if let Some((manufacturer, size)) = original {
        (
            uses.add_value_origins(manufacturer, &[attach_def_present.origin()])?,
            uses.add_value_origins(size, &[attach_def_present.origin()])?,
        )
    } else {
        let missing = attach_def_present.clone().try_map(|_| {
            Err::<(ManufacturerValue<'source>, ResourceNumber), CatalogError>(
                ExportAccountingError::ConflictingSourceDatum.into(),
            )
        })?;
        missing.unzip()
    };
    Ok(AttachmentRead {
        manufacturer: ShipAttachmentManufacturer {
            component_ordinal: uses.add_value_origins(source.array_ordinal()?, &[&placement])?,
            attach_def_present,
            manufacturer,
        },
        size,
    })
}

impl<'source> ShipVehicleManufacturer<'source> {
    /// Borrows the original selected vehicle-component position.
    #[must_use]
    pub const fn component_ordinal(&self) -> &SourcedValue<u32> {
        &self.component_ordinal
    }

    /// Borrows the authored manufacturer value.
    #[must_use]
    pub const fn manufacturer(&self) -> &SourcedValue<ManufacturerValue<'source>> {
        &self.manufacturer
    }
}

impl<'source> ShipAttachmentManufacturer<'source> {
    /// Borrows the original selected attachment-component position.
    #[must_use]
    pub const fn component_ordinal(&self) -> &SourcedValue<u32> {
        &self.component_ordinal
    }

    /// Borrows the original attachment-definition state.
    #[must_use]
    pub const fn attach_def_present(&self) -> &SourcedValue<bool> {
        &self.attach_def_present
    }

    /// Borrows the authored attachment manufacturer.
    #[must_use]
    pub const fn manufacturer(&self) -> &SourcedValue<ManufacturerValue<'source>> {
        &self.manufacturer
    }
}
