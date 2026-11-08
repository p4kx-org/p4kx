// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, StructValue};

use crate::CatalogError;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::resource_density_source::{
    ResourceDensity, ResourceDensityPayload, ResourceDensitySource, ResourceDensityUnit,
    ResourceDensityUnitPayload, ResourceDensityValue, ResourceMolarMass,
};
use crate::resource_value_read;
use crate::source_fields::FieldError;

/// Projects the owned density shape without changing its unit or IEEE values.
pub(crate) fn read<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceDensitySource<'f>, CatalogError> {
    let fields = CatalogFields::new(source, path, context);
    let payload = match source.type_name() {
        "ResourceTypeDensityType" => {
            fields.check(&[], notes);
            Some(ResourceDensityPayload::EmptyBase)
        }
        "ResourceTypeDensity" => {
            fields.check(&["densityUnit"], notes);
            let density_unit = fields
                .structure("densityUnit", notes)?
                .try_map(|unit| read_unit(unit, &format!("{path}.densityUnit"), context, notes))?;
            Some(ResourceDensityPayload::Density(ResourceDensity {
                density_unit,
            }))
        }
        "ResourceTypeMolarMass" => {
            fields.check(&["molarMass"], notes);
            Some(ResourceDensityPayload::MolarMass(ResourceMolarMass {
                molar_mass: resource_value_read::single(&fields, "molarMass", notes)?,
            }))
        }
        _ => None,
    };
    Ok(ResourceDensitySource {
        source_type: fields.source_type()?,
        payload: payload.map_or_else(
            || fields.unsupported(notes),
            |payload| context.selector(&format!("{path}.$type"), SourceValue::Present(payload)),
        )?,
    })
}

fn read_unit<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceDensityUnit<'f>, CatalogError> {
    let fields = CatalogFields::new(source, path, context);
    let payload = match source.type_name() {
        "BaseDensityUnit" => {
            fields.check(&[], notes);
            Some(ResourceDensityUnitPayload::EmptyBase)
        }
        "GramsPerCubicCentimeter" => {
            fields.check(&["gramsPerCubicCentimeter"], notes);
            Some(ResourceDensityUnitPayload::GramsPerCubicCentimeter(
                ResourceDensityValue {
                    value: resource_value_read::single(&fields, "gramsPerCubicCentimeter", notes)?,
                },
            ))
        }
        "KilogramsPerCubicMeter" => {
            fields.check(&["kilogramsPerCubicMeter"], notes);
            Some(ResourceDensityUnitPayload::KilogramsPerCubicMeter(
                ResourceDensityValue {
                    value: resource_value_read::single(&fields, "kilogramsPerCubicMeter", notes)?,
                },
            ))
        }
        _ => None,
    };
    Ok(ResourceDensityUnit {
        source_type: fields.source_type()?,
        payload: payload.map_or_else(
            || fields.unsupported(notes),
            |payload| context.selector(&format!("{path}.$type"), SourceValue::Present(payload)),
        )?,
    })
}
