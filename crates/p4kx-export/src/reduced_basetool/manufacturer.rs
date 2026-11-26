// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use crate::catalog_v3::authored_fields::ship_presentation::{
    ShipAttachmentManufacturer, ShipVehicleManufacturer,
};
use crate::definition_reference::DefinitionStatus;
use crate::guid_text::CatalogGuid;
use crate::source_value_catalog::{
    SourceArenaOutput, SourcePayload, SourceRecordReference, SourceText,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Manufacturer<'source> {
    component_ordinal: Cell<&'source u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    attach_def_present: Option<Cell<&'source bool>>,
    manufacturer: Cell<ManufacturerValue<'source>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "value")]
enum ManufacturerValue<'source> {
    Record(Record<'source>),
    String(Cell<&'source SourceText<'source>>),
    Locale(Cell<&'source SourceText<'source>>),
    Enum(Cell<&'source str>),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Record<'source> {
    target_record_index: Cell<&'source u32>,
    guid: Cell<&'source CatalogGuid>,
    record_path: Cell<&'source str>,
    target_type: Cell<&'source str>,
    status: &'source DefinitionStatus,
}

impl<'source> From<&'source SourceRecordReference<'source>> for Record<'source> {
    fn from(reference: &'source SourceRecordReference<'source>) -> Self {
        Self {
            target_record_index: Cell::from(reference.target_record_index()),
            guid: Cell::from(&reference.guid),
            record_path: Cell::from(reference.record_path()).map(|path| *path),
            target_type: Cell::from(reference.target_type()).map(|name| *name),
            status: reference.status(),
        }
    }
}

impl<'source> Manufacturer<'source> {
    pub(super) fn vehicle(
        input: &'source ShipVehicleManufacturer<'source>,
        record_index: u32,
        verify: impl FnOnce(&SourceRecordReference<'_>) -> Result<(), ReducedError>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            component_ordinal: Cell::from(input.component_ordinal()),
            attach_def_present: None,
            manufacturer: project(input.manufacturer(), record_index, verify)?,
        })
    }

    pub(super) fn attachment(
        input: &'source ShipAttachmentManufacturer<'source>,
        record_index: u32,
        verify: impl FnOnce(&SourceRecordReference<'_>) -> Result<(), ReducedError>,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            component_ordinal: Cell::from(input.component_ordinal()),
            attach_def_present: Some(Cell::from(input.attach_def_present())),
            manufacturer: project(input.manufacturer(), record_index, verify)?,
        })
    }
}

fn project<'source>(
    value: &'source SourcedValue<
        crate::catalog_v3::authored_fields::authored_manufacturer::ManufacturerValue<'source>,
    >,
    record_index: u32,
    verify: impl FnOnce(&SourceRecordReference<'_>) -> Result<(), ReducedError>,
) -> Result<Cell<ManufacturerValue<'source>>, ReducedError> {
    use crate::catalog_v3::authored_fields::authored_manufacturer::ManufacturerValue as Original;

    Cell::from(value).try_map(|value| match value {
        Original::Record(reference) => {
            verify(reference)?;
            Ok(ManufacturerValue::Record(Record::from(reference.as_ref())))
        }
        Original::Authored { authored } => scalar(authored, record_index),
    })
}

fn scalar<'source>(
    arena: &'source SourceArenaOutput<'source>,
    record_index: u32,
) -> Result<ManufacturerValue<'source>, ReducedError> {
    let [root] = arena.roots() else {
        return Err(ReducedError::UnsupportedManufacturer(record_index));
    };
    let value = arena
        .value(root.value())
        .ok_or(ReducedError::UnsupportedManufacturer(record_index))?
        .value();
    let cell = Cell::from(value);
    match cell.value {
        Some(SourcePayload::String { value }) => Ok(ManufacturerValue::String(cell.map(|_| value))),
        Some(SourcePayload::Locale { value }) => Ok(ManufacturerValue::Locale(cell.map(|_| value))),
        Some(SourcePayload::Enum { value }) => Ok(ManufacturerValue::Enum(cell.map(|_| *value))),
        _ => Err(ReducedError::UnsupportedManufacturer(record_index)),
    }
}
