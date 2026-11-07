// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::resource_density_source::{
    ResourceDensity, ResourceDensityPayload, ResourceDensitySource, ResourceDensityUnit,
    ResourceDensityUnitPayload, ResourceDensityValue, ResourceMolarMass,
};
use crate::source_fields::{FieldError, Fields};

/// Projects an authored density quantity without normalising it into a commodity field.
pub(crate) fn read<'source>(
    source: &StructValue<'source>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceDensitySource<'source>, FieldError> {
    let fields = Fields::new(source, path);
    let payload = match source.type_name() {
        "ResourceTypeDensityType" => {
            fields.check(&[], notes);
            ResourceDensityPayload::EmptyBase
        }
        "ResourceTypeDensity" => {
            fields.check(&["densityUnit"], notes);
            let density_unit = fields
                .structure("densityUnit")?
                .map(|unit| read_unit(unit, &format!("{path}.densityUnit"), notes))
                .transpose()?;
            ResourceDensityPayload::Density(ResourceDensity { density_unit })
        }
        "ResourceTypeMolarMass" => {
            fields.check(&["molarMass"], notes);
            ResourceDensityPayload::MolarMass(ResourceMolarMass {
                molar_mass: fields.single("molarMass")?,
            })
        }
        _ => {
            notes.push(fields.unsupported(""));
            ResourceDensityPayload::Unsupported
        }
    };
    Ok(ResourceDensitySource {
        source_type: source.type_name(),
        payload,
    })
}

fn read_unit<'source>(
    source: &StructValue<'source>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceDensityUnit<'source>, FieldError> {
    let fields = Fields::new(source, path);
    let payload = match source.type_name() {
        "BaseDensityUnit" => {
            fields.check(&[], notes);
            ResourceDensityUnitPayload::EmptyBase
        }
        "GramsPerCubicCentimeter" => {
            fields.check(&["gramsPerCubicCentimeter"], notes);
            ResourceDensityUnitPayload::GramsPerCubicCentimeter(ResourceDensityValue {
                value: fields.single("gramsPerCubicCentimeter")?,
            })
        }
        "KilogramsPerCubicMeter" => {
            fields.check(&["kilogramsPerCubicMeter"], notes);
            ResourceDensityUnitPayload::KilogramsPerCubicMeter(ResourceDensityValue {
                value: fields.single("kilogramsPerCubicMeter")?,
            })
        }
        _ => {
            notes.push(fields.unsupported(""));
            ResourceDensityUnitPayload::Unsupported
        }
    };
    Ok(ResourceDensityUnit {
        source_type: source.type_name(),
        payload,
    })
}
