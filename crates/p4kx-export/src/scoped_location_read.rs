// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::record_read::ReadError;
use crate::scoped_location::{
    ClassFilter, ClassTarget, ClassTargets, FactionFilter, FunctionFilter, FunctionTarget,
    FunctionTargets, Hostility, OtherLocationFilter, PropertyTarget, PropertyTargets,
    RelativeLocationFilter, RelativeTarget, ScopedLocation, SizeFilter, SizeTarget, SizeTargets,
};
use crate::scoped_location_fields as fields;
use crate::scoped_location_kind::ScopedLocationFilterKind;
use crate::source_fields::{FieldCapture, unknown_struct};
use crate::spawn_read::Reader;

/// Reads every declared `ScopedLocation` filter without selecting a runtime location.
///
/// # Errors
///
/// Returns a strict variant or source-field error instead of treating unreadable data as absent.
pub(crate) fn read<'source>(
    reader: &Reader<'_, 'source>,
    value: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<ScopedLocation, ReadError> {
    if let Some(capture) = capture {
        capture.check_parent(value)?;
    }
    Ok(ScopedLocation {
        faction: filter(value, "factionFilter", capture, |value| {
            faction(value, capture)
        })?,
        class: filter(value, "locationClassFilter", capture, |value| {
            class(value, capture)
        })?,
        size: filter(value, "locationSizeFilter", capture, |value| {
            size(value, capture)
        })?,
        function: filter(value, "locationFunctionFilter", capture, |value| {
            function(value, capture)
        })?,
        other: filter(value, "otherLocationFilter", capture, |value| {
            other(reader, value, capture)
        })?,
        relative: filter(value, "relativeLocationFilter", capture, |value| {
            relative(reader, value, capture)
        })?,
    })
}

fn filter<'source, T>(
    parent: &StructValue<'source>,
    name: &'static str,
    capture: Option<&dyn FieldCapture<'source>>,
    read: impl FnOnce(&StructValue<'source>) -> Result<T, ReadError>,
) -> Result<Option<T>, ReadError> {
    fields::structure(parent, name, capture)?
        .map(read)
        .transpose()
}

fn faction<'source>(
    value: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<FactionFilter, ReadError> {
    let payload = Hostility {
        hostility: fields::enum_name(value, "hostility", capture)?,
    };
    Ok(match ScopedLocationFilterKind::read(value.type_name()) {
        Some(ScopedLocationFilterKind::RequiredFactionHostility) => {
            FactionFilter::RequiredFactionHostility(payload)
        }
        Some(ScopedLocationFilterKind::ExcludedFactionHostility) => {
            FactionFilter::ExcludedFactionHostility(payload)
        }
        _ => return Err(unknown_struct(value, "factionFilter", capture)),
    })
}

fn class<'source>(
    value: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<ClassFilter, ReadError> {
    Ok(match ScopedLocationFilterKind::read(value.type_name()) {
        Some(ScopedLocationFilterKind::RequiredLocationClass) => {
            ClassFilter::RequiredLocationClass(ClassTarget {
                location_class: fields::definition(value, "locationClass", capture)?,
            })
        }
        Some(ScopedLocationFilterKind::RequiredLocationClasses) => {
            ClassFilter::RequiredLocationClasses(ClassTargets {
                location_classes: fields::definitions(value, "locationClasses", capture)?,
            })
        }
        Some(ScopedLocationFilterKind::ExcludedLocationClass) => {
            ClassFilter::ExcludedLocationClass(ClassTarget {
                location_class: fields::definition(value, "locationClass", capture)?,
            })
        }
        Some(ScopedLocationFilterKind::ExcludedLocationClasses) => {
            ClassFilter::ExcludedLocationClasses(ClassTargets {
                location_classes: fields::definitions(value, "locationClasses", capture)?,
            })
        }
        _ => return Err(unknown_struct(value, "locationClassFilter", capture)),
    })
}

fn size<'source>(
    value: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<SizeFilter, ReadError> {
    Ok(match ScopedLocationFilterKind::read(value.type_name()) {
        Some(ScopedLocationFilterKind::RequiredLocationSize) => {
            SizeFilter::RequiredLocationSize(SizeTarget {
                location_size: fields::definition(value, "locationSize", capture)?,
            })
        }
        Some(ScopedLocationFilterKind::RequiredLocationSizes) => {
            SizeFilter::RequiredLocationSizes(SizeTargets {
                location_sizes: fields::definitions(value, "locationSizes", capture)?,
            })
        }
        Some(ScopedLocationFilterKind::ExcludedLocationSize) => {
            SizeFilter::ExcludedLocationSize(SizeTarget {
                location_size: fields::definition(value, "locationSize", capture)?,
            })
        }
        Some(ScopedLocationFilterKind::ExcludedLocationSizes) => {
            SizeFilter::ExcludedLocationSizes(SizeTargets {
                location_sizes: fields::definitions(value, "locationSizes", capture)?,
            })
        }
        _ => return Err(unknown_struct(value, "locationSizeFilter", capture)),
    })
}

fn function<'source>(
    value: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<FunctionFilter, ReadError> {
    Ok(match ScopedLocationFilterKind::read(value.type_name()) {
        Some(ScopedLocationFilterKind::RequiredLocationFunction) => {
            FunctionFilter::RequiredLocationFunction(FunctionTarget {
                location_function: fields::definition(value, "locationFunction", capture)?,
            })
        }
        Some(ScopedLocationFilterKind::RequiredLocationFunctions) => {
            FunctionFilter::RequiredLocationFunctions(FunctionTargets {
                location_functions: fields::definitions(value, "locationFunctions", capture)?,
            })
        }
        Some(ScopedLocationFilterKind::ExcludedLocationFunction) => {
            FunctionFilter::ExcludedLocationFunction(FunctionTarget {
                location_function: fields::definition(value, "locationFunction", capture)?,
            })
        }
        Some(ScopedLocationFilterKind::ExcludedLocationFunctions) => {
            FunctionFilter::ExcludedLocationFunctions(FunctionTargets {
                location_functions: fields::definitions(value, "locationFunctions", capture)?,
            })
        }
        _ => return Err(unknown_struct(value, "locationFunctionFilter", capture)),
    })
}

fn other<'source>(
    reader: &Reader<'_, 'source>,
    value: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<OtherLocationFilter, ReadError> {
    Ok(match ScopedLocationFilterKind::read(value.type_name()) {
        Some(ScopedLocationFilterKind::NotLocation) => {
            OtherLocationFilter::NotLocation(PropertyTarget {
                location: fields::property(reader, value, "location", capture)?,
            })
        }
        Some(ScopedLocationFilterKind::SameLocation) => {
            OtherLocationFilter::SameLocation(PropertyTarget {
                location: fields::property(reader, value, "location", capture)?,
            })
        }
        Some(ScopedLocationFilterKind::NotLocations) => {
            OtherLocationFilter::NotLocations(PropertyTargets {
                locations: fields::properties(reader, value, "locations", capture)?,
            })
        }
        _ => return Err(unknown_struct(value, "otherLocationFilter", capture)),
    })
}

fn relative<'source>(
    reader: &Reader<'_, 'source>,
    value: &StructValue<'source>,
    capture: Option<&dyn FieldCapture<'source>>,
) -> Result<RelativeLocationFilter, ReadError> {
    let payload = RelativeTarget {
        scope: fields::enum_name(value, "scope", capture)?,
        location: fields::property(reader, value, "location", capture)?,
    };
    Ok(match ScopedLocationFilterKind::read(value.type_name()) {
        Some(ScopedLocationFilterKind::SameScope) => RelativeLocationFilter::SameScope(payload),
        Some(ScopedLocationFilterKind::DifferentScope) => {
            RelativeLocationFilter::DifferentScope(payload)
        }
        _ => return Err(unknown_struct(value, "relativeLocationFilter", capture)),
    })
}
