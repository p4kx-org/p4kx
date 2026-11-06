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
use crate::spawn_read::Reader;
use crate::template_issues::TemplateError;

/// Reads every declared `ScopedLocation` filter without selecting a runtime location.
///
/// # Errors
///
/// Returns a strict variant or source-field error instead of treating unreadable data as absent.
pub(crate) fn read(
    reader: &Reader<'_, '_>,
    value: &StructValue<'_>,
) -> Result<ScopedLocation, ReadError> {
    Ok(ScopedLocation {
        faction: filter(value, "factionFilter", faction)?,
        class: filter(value, "locationClassFilter", class)?,
        size: filter(value, "locationSizeFilter", size)?,
        function: filter(value, "locationFunctionFilter", function)?,
        other: filter(value, "otherLocationFilter", |value| other(reader, value))?,
        relative: filter(value, "relativeLocationFilter", |value| {
            relative(reader, value)
        })?,
    })
}

fn filter<T>(
    parent: &StructValue<'_>,
    name: &'static str,
    read: impl FnOnce(&StructValue<'_>) -> Result<T, ReadError>,
) -> Result<Option<T>, ReadError> {
    fields::structure(parent, name)?.map(read).transpose()
}

fn faction(value: &StructValue<'_>) -> Result<FactionFilter, ReadError> {
    let payload = Hostility {
        hostility: fields::enum_name(value, "hostility")?,
    };
    Ok(match value.type_name() {
        "LocationCondition_RequiredFactionHostility" => {
            FactionFilter::RequiredFactionHostility(payload)
        }
        "LocationCondition_ExcludedFactionHostility" => {
            FactionFilter::ExcludedFactionHostility(payload)
        }
        _ => return Err(TemplateError::unknown("factionFilter", value).into()),
    })
}

fn class(value: &StructValue<'_>) -> Result<ClassFilter, ReadError> {
    Ok(match value.type_name() {
        "LocationCondition_RequiredLocationClass" => {
            ClassFilter::RequiredLocationClass(ClassTarget {
                location_class: fields::definition(value, "locationClass")?,
            })
        }
        "LocationCondition_RequiredLocationClasses" => {
            ClassFilter::RequiredLocationClasses(ClassTargets {
                location_classes: fields::definitions(value, "locationClasses")?,
            })
        }
        "LocationCondition_ExcludedLocationClass" => {
            ClassFilter::ExcludedLocationClass(ClassTarget {
                location_class: fields::definition(value, "locationClass")?,
            })
        }
        "LocationCondition_ExcludedLocationClasses" => {
            ClassFilter::ExcludedLocationClasses(ClassTargets {
                location_classes: fields::definitions(value, "locationClasses")?,
            })
        }
        _ => return Err(TemplateError::unknown("locationClassFilter", value).into()),
    })
}

fn size(value: &StructValue<'_>) -> Result<SizeFilter, ReadError> {
    Ok(match value.type_name() {
        "LocationCondition_RequiredLocationSize" => SizeFilter::RequiredLocationSize(SizeTarget {
            location_size: fields::definition(value, "locationSize")?,
        }),
        "LocationCondition_RequiredLocationSizes" => {
            SizeFilter::RequiredLocationSizes(SizeTargets {
                location_sizes: fields::definitions(value, "locationSizes")?,
            })
        }
        "LocationCondition_ExcludedLocationSize" => SizeFilter::ExcludedLocationSize(SizeTarget {
            location_size: fields::definition(value, "locationSize")?,
        }),
        "LocationCondition_ExcludedLocationSizes" => {
            SizeFilter::ExcludedLocationSizes(SizeTargets {
                location_sizes: fields::definitions(value, "locationSizes")?,
            })
        }
        _ => return Err(TemplateError::unknown("locationSizeFilter", value).into()),
    })
}

fn function(value: &StructValue<'_>) -> Result<FunctionFilter, ReadError> {
    Ok(match value.type_name() {
        "LocationCondition_RequiredLocationFunction" => {
            FunctionFilter::RequiredLocationFunction(FunctionTarget {
                location_function: fields::definition(value, "locationFunction")?,
            })
        }
        "LocationCondition_RequiredLocationFunctions" => {
            FunctionFilter::RequiredLocationFunctions(FunctionTargets {
                location_functions: fields::definitions(value, "locationFunctions")?,
            })
        }
        "LocationCondition_ExcludedLocationFunction" => {
            FunctionFilter::ExcludedLocationFunction(FunctionTarget {
                location_function: fields::definition(value, "locationFunction")?,
            })
        }
        "LocationCondition_ExcludedLocationFunctions" => {
            FunctionFilter::ExcludedLocationFunctions(FunctionTargets {
                location_functions: fields::definitions(value, "locationFunctions")?,
            })
        }
        _ => return Err(TemplateError::unknown("locationFunctionFilter", value).into()),
    })
}

fn other(
    reader: &Reader<'_, '_>,
    value: &StructValue<'_>,
) -> Result<OtherLocationFilter, ReadError> {
    Ok(match value.type_name() {
        "LocationCondition_NotLocation" => OtherLocationFilter::NotLocation(PropertyTarget {
            location: fields::property(reader, value, "location")?,
        }),
        "LocationCondition_SameLocation" => OtherLocationFilter::SameLocation(PropertyTarget {
            location: fields::property(reader, value, "location")?,
        }),
        "LocationCondition_NotLocations" => OtherLocationFilter::NotLocations(PropertyTargets {
            locations: fields::properties(reader, value, "locations")?,
        }),
        _ => return Err(TemplateError::unknown("otherLocationFilter", value).into()),
    })
}

fn relative(
    reader: &Reader<'_, '_>,
    value: &StructValue<'_>,
) -> Result<RelativeLocationFilter, ReadError> {
    let payload = RelativeTarget {
        scope: fields::enum_name(value, "scope")?,
        location: fields::property(reader, value, "location")?,
    };
    Ok(match value.type_name() {
        "LocationCondition_SameScope" => RelativeLocationFilter::SameScope(payload),
        "LocationCondition_DifferentScope" => RelativeLocationFilter::DifferentScope(payload),
        _ => return Err(TemplateError::unknown("relativeLocationFilter", value).into()),
    })
}
