// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::Value;

use crate::CatalogError;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::scoped_location_kind::ScopedLocationFilterKind;
use crate::template_property_kind::PropertyValueKind;
use crate::template_property_plan::SelectedProperty;

/// Actual selected fields whose established interpretation requires a `MissionProperty` target.
pub(crate) fn read<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    properties: &[SelectedProperty<'_, 'scope, 'read, 'source>],
) -> Result<HashSet<u64>, CatalogError> {
    let mut slots = HashSet::new();
    for property in properties {
        fields.check_cancel()?;
        let Some(value) = property.value else {
            continue;
        };
        let Value::Struct(source) = value.value() else {
            continue;
        };
        match PropertyValueKind::read(source.type_name()) {
            Some(PropertyValueKind::ScopedLocation) => scoped(fields, value, &mut slots)?,
            Some(
                kind @ (PropertyValueKind::StringHash
                | PropertyValueKind::Integer
                | PropertyValueKind::Float
                | PropertyValueKind::Tags),
            ) => options(fields, value, kind, &mut slots)?,
            _ => {}
        }
    }
    Ok(slots)
}

fn scoped<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    value: &OriginalValueFact<'scope, 'read, 'source>,
    slots: &mut HashSet<u64>,
) -> Result<(), CatalogError> {
    for name in ["otherLocationFilter", "relativeLocationFilter"] {
        let Some((_, filter)) = fields.field(Some(value), name)? else {
            continue;
        };
        let Value::Struct(source) = filter.value() else {
            continue;
        };
        let field = match (name, ScopedLocationFilterKind::read(source.type_name())) {
            (
                "otherLocationFilter",
                Some(
                    ScopedLocationFilterKind::NotLocation | ScopedLocationFilterKind::SameLocation,
                ),
            )
            | (
                "relativeLocationFilter",
                Some(
                    ScopedLocationFilterKind::SameScope | ScopedLocationFilterKind::DifferentScope,
                ),
            ) => "location",
            ("otherLocationFilter", Some(ScopedLocationFilterKind::NotLocations)) => "locations",
            _ => continue,
        };
        if let Some((_, location)) = fields.field(Some(filter), field)? {
            retain(fields, location, slots)?;
        }
    }
    Ok(())
}

fn options<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    value: &OriginalValueFact<'scope, 'read, 'source>,
    kind: PropertyValueKind,
    slots: &mut HashSet<u64>,
) -> Result<(), CatalogError> {
    let Some((_, options)) = fields.field(Some(value), "options")? else {
        return Ok(());
    };
    for option in fields.structs(options)? {
        let Value::Struct(source) = option.value() else {
            continue;
        };
        if Some(source.type_name()) != kind.option_type() {
            continue;
        }
        let Some((_, declarations)) = fields.field(Some(option), "dependentProperties")? else {
            continue;
        };
        for dependency in fields.structs(declarations)? {
            if let Some((_, caller)) = fields.field(Some(dependency), "property")? {
                retain(fields, caller, slots)?;
            }
        }
    }
    Ok(())
}

fn retain<'scope, 'read, 'source>(
    fields: &OriginalFlowFields<'scope, 'read, 'source>,
    fact: &OriginalValueFact<'scope, 'read, 'source>,
    slots: &mut HashSet<u64>,
) -> Result<(), CatalogError> {
    match fact.value() {
        Value::Array(_) => {
            for element in fields.elements(fact)? {
                slots.insert(element.ordinal());
            }
        }
        _ => {
            slots.insert(fact.ordinal());
        }
    }
    Ok(())
}
