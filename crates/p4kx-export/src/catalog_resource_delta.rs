// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_resource_delta_amount::{
    RESOURCE_ENUM, ResourceComposition, ResourceDeltaAmount,
};
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

type Side<'source> = SourcedValue<ResourceDeltaAmount<'source>>;
type Composition<'source> = SourcedValue<ResourceComposition<'source>>;

/// A source delta with its original array slot and linked sides.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(super) enum ResourceDelta<'source> {
    Consumption {
        #[serde(flatten)]
        source: DeltaSource<'source>,
        #[serde(flatten)]
        fields: ConsumptionFields<'source>,
    },
    Generation {
        #[serde(flatten)]
        source: DeltaSource<'source>,
        generation: Side<'source>,
        composition: Composition<'source>,
        no_over_generation: SourcedValue<bool>,
    },
    Conversion {
        #[serde(flatten)]
        source: DeltaSource<'source>,
        #[serde(flatten)]
        consumed: ConsumptionFields<'source>,
        generation: Side<'source>,
        generated_composition: Composition<'source>,
        no_over_generation: SourcedValue<bool>,
    },
    Storage {
        #[serde(flatten)]
        source: DeltaSource<'source>,
        consumption: Side<'source>,
        generation: Side<'source>,
        consumption_composition: Composition<'source>,
        discharge: SourcedValue<bool>,
    },
    NetworkReflection {
        #[serde(flatten)]
        source: DeltaSource<'source>,
        resource: SourcedValue<&'source str>,
        binary_evaluation: SourcedValue<bool>,
    },
    UnknownDelta {
        #[serde(flatten)]
        source: DeltaSource<'source>,
    },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct DeltaSource<'source> {
    source_type: SourcedValue<&'source str>,
    source_position: SourcedValue<u32>,
    additional_fields: SourceArenaOutput<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ConsumptionFields<'source> {
    minimum_consumption_fraction: SourcedValue<ResourceFloat32>,
    consumption: Side<'source>,
    consumption_composition: Composition<'source>,
}

impl<'source> ResourceDelta<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        match source.source_type()? {
            "ItemResourceDeltaConsumption" => {
                let fields = ConsumptionFields::read(source, notes)?;
                Ok(Self::Consumption {
                    source: DeltaSource::read(source, fields.projected())?,
                    fields,
                })
            }
            "ItemResourceDeltaGeneration" => Self::generation(source, notes),
            "ItemResourceDeltaConversion" => Self::conversion(source, notes),
            "ItemResourceDeltaStorage" => Self::storage(source, notes),
            "ItemResourceDeltaNetworkReflection" => Self::network_reflection(source, notes),
            _ => Ok(Self::UnknownDelta {
                source: DeltaSource::read(source, [])?,
            }),
        }
    }

    fn generation(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let generation = source.declared_class("generation", notes, ResourceDeltaAmount::read)?;
        let composition = source.declared_class("composition", notes, ResourceComposition::read)?;
        let no_over_generation = source.boolean("noOverGeneration", notes)?;
        Ok(Self::Generation {
            source: DeltaSource::read(
                source,
                [
                    ("generation", generation.state()),
                    ("composition", composition.state()),
                    ("noOverGeneration", no_over_generation.state()),
                ],
            )?,
            generation,
            composition,
            no_over_generation,
        })
    }

    fn conversion(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let consumed = ConsumptionFields::read(source, notes)?;
        let generation = source.declared_class("generation", notes, ResourceDeltaAmount::read)?;
        let generated_composition =
            source.declared_class("generatedComposition", notes, ResourceComposition::read)?;
        let no_over_generation = source.boolean("noOverGeneration", notes)?;
        Ok(Self::Conversion {
            source: DeltaSource::read(
                source,
                consumed.projected().into_iter().chain([
                    ("generation", generation.state()),
                    ("generatedComposition", generated_composition.state()),
                    ("noOverGeneration", no_over_generation.state()),
                ]),
            )?,
            consumed,
            generation,
            generated_composition,
            no_over_generation,
        })
    }

    fn storage(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let consumption = source.declared_class("consumption", notes, ResourceDeltaAmount::read)?;
        let generation = source.declared_class("generation", notes, ResourceDeltaAmount::read)?;
        let consumption_composition =
            source.declared_class("consumptionComposition", notes, ResourceComposition::read)?;
        let discharge = source.boolean("discharge", notes)?;
        Ok(Self::Storage {
            source: DeltaSource::read(
                source,
                [
                    ("consumption", consumption.state()),
                    ("generation", generation.state()),
                    ("consumptionComposition", consumption_composition.state()),
                    ("discharge", discharge.state()),
                ],
            )?,
            consumption,
            generation,
            consumption_composition,
            discharge,
        })
    }

    fn network_reflection(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let resource = source.borrowed_enum_or_string("resource", RESOURCE_ENUM, notes)?;
        let binary_evaluation = source.boolean("binaryEvaluation", notes)?;
        Ok(Self::NetworkReflection {
            source: DeltaSource::read(
                source,
                [
                    ("resource", resource.state()),
                    ("binaryEvaluation", binary_evaluation.state()),
                ],
            )?,
            resource,
            binary_evaluation,
        })
    }
}

impl<'source> DeltaSource<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        projected: impl IntoIterator<Item = (&'static str, SourceValueState)>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source_type: source.source_type_value()?,
            source_position: source.array_ordinal()?,
            additional_fields: source.authored_members_except(projected)?,
        })
    }
}

impl<'source> ConsumptionFields<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            minimum_consumption_fraction: source.single("minimumConsumptionFraction", notes)?,
            consumption: source.declared_class("consumption", notes, ResourceDeltaAmount::read)?,
            consumption_composition: source.declared_class(
                "consumptionComposition",
                notes,
                ResourceComposition::read,
            )?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 3] {
        [
            (
                "minimumConsumptionFraction",
                self.minimum_consumption_fraction.state(),
            ),
            ("consumption", self.consumption.state()),
            (
                "consumptionComposition",
                self.consumption_composition.state(),
            ),
        ]
    }
}
