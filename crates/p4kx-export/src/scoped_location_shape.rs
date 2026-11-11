// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::record_read::ReadError;
use crate::scoped_location_fields::ShapeFields;
use crate::scoped_location_kind::ScopedLocationFilterKind;
use crate::source_fields::{FieldCapture, unknown_struct};

/// Checks the existing six filter families without copying their raw payloads.
pub(crate) fn validate_observed<'source>(
    source: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    let fields = ShapeFields::new(source, capture)?;
    if let Some(value) = fields.structure("factionFilter")? {
        ShapeFields::new(value, capture)?.enum_view("hostility")?;
        match ScopedLocationFilterKind::read(value.type_name()) {
            Some(
                ScopedLocationFilterKind::RequiredFactionHostility
                | ScopedLocationFilterKind::ExcludedFactionHostility,
            ) => {}
            _ => return Err(unknown_struct(value, "factionFilter", Some(capture))),
        }
    }
    if let Some(value) = fields.structure("locationClassFilter")? {
        class(value, capture)?;
    }
    if let Some(value) = fields.structure("locationSizeFilter")? {
        size(value, capture)?;
    }
    if let Some(value) = fields.structure("locationFunctionFilter")? {
        function(value, capture)?;
    }
    if let Some(value) = fields.structure("otherLocationFilter")? {
        other(value, capture)?;
    }
    if let Some(value) = fields.structure("relativeLocationFilter")? {
        relative(value, capture)?;
    }
    Ok(())
}

fn class<'source>(
    value: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    let fields = ShapeFields::new(value, capture)?;
    match ScopedLocationFilterKind::read(value.type_name()) {
        Some(
            ScopedLocationFilterKind::RequiredLocationClass
            | ScopedLocationFilterKind::ExcludedLocationClass,
        ) => fields.definition("locationClass"),
        Some(
            ScopedLocationFilterKind::RequiredLocationClasses
            | ScopedLocationFilterKind::ExcludedLocationClasses,
        ) => fields.definitions("locationClasses"),
        _ => Err(unknown_struct(value, "locationClassFilter", Some(capture))),
    }
}

fn size<'source>(
    value: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    let fields = ShapeFields::new(value, capture)?;
    match ScopedLocationFilterKind::read(value.type_name()) {
        Some(
            ScopedLocationFilterKind::RequiredLocationSize
            | ScopedLocationFilterKind::ExcludedLocationSize,
        ) => fields.definition("locationSize"),
        Some(
            ScopedLocationFilterKind::RequiredLocationSizes
            | ScopedLocationFilterKind::ExcludedLocationSizes,
        ) => fields.definitions("locationSizes"),
        _ => Err(unknown_struct(value, "locationSizeFilter", Some(capture))),
    }
}

fn function<'source>(
    value: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    let fields = ShapeFields::new(value, capture)?;
    match ScopedLocationFilterKind::read(value.type_name()) {
        Some(
            ScopedLocationFilterKind::RequiredLocationFunction
            | ScopedLocationFilterKind::ExcludedLocationFunction,
        ) => fields.definition("locationFunction"),
        Some(
            ScopedLocationFilterKind::RequiredLocationFunctions
            | ScopedLocationFilterKind::ExcludedLocationFunctions,
        ) => fields.definitions("locationFunctions"),
        _ => Err(unknown_struct(
            value,
            "locationFunctionFilter",
            Some(capture),
        )),
    }
}

fn other<'source>(
    value: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    let fields = ShapeFields::new(value, capture)?;
    match ScopedLocationFilterKind::read(value.type_name()) {
        Some(ScopedLocationFilterKind::NotLocation | ScopedLocationFilterKind::SameLocation) => {
            fields.property("location")
        }
        Some(ScopedLocationFilterKind::NotLocations) => fields.properties("locations"),
        _ => Err(unknown_struct(value, "otherLocationFilter", Some(capture))),
    }
}

fn relative<'source>(
    value: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    let fields = ShapeFields::new(value, capture)?;
    fields.enum_view("scope")?;
    fields.property("location")?;
    match ScopedLocationFilterKind::read(value.type_name()) {
        Some(ScopedLocationFilterKind::SameScope | ScopedLocationFilterKind::DifferentScope) => {
            Ok(())
        }
        _ => Err(unknown_struct(
            value,
            "relativeLocationFilter",
            Some(capture),
        )),
    }
}
