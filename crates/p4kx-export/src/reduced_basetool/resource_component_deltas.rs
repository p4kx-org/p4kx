// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use serde::Serialize;

use super::super::ReducedError;
use super::super::cell::Cell;
use super::super::completeness;
use super::super::number::Number;
use super::super::record_link::RecordLink;
use super::super::record_targets::RecordTargets;
use super::PATH;
use crate::catalog_queries::equipment::resource_network::{
    CompositionValue, ConsumptionFields, ResourceComposition, ResourceDelta, ResourceDeltaAmount,
    ResourceFlowUnit,
};
use crate::resource_number::ResourceNumber;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Delta<'read> {
    source_type: Cell<&'read str>,
    source_position: Cell<&'read u32>,
    #[serde(flatten)]
    kind: Kind<'read>,
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Kind<'read> {
    Consumption {
        #[serde(flatten)]
        consumed: ResourceConsumption<'read>,
    },
    Generation {
        generation: Cell<Amount<'read>>,
        composition: Cell<Composition<'read>>,
        no_over_generation: Cell<&'read bool>,
    },
    Conversion {
        #[serde(flatten)]
        consumed: ResourceConsumption<'read>,
        generation: Cell<Amount<'read>>,
        generated_composition: Cell<Composition<'read>>,
        no_over_generation: Cell<&'read bool>,
    },
    Storage {
        consumption: Cell<Amount<'read>>,
        generation: Cell<Amount<'read>>,
        consumption_composition: Cell<Composition<'read>>,
        discharge: Cell<&'read bool>,
    },
    NetworkReflection {
        resource: Cell<&'read str>,
        binary_evaluation: Cell<&'read bool>,
    },
    UnknownDelta,
}

impl<'read> Delta<'read> {
    pub(super) fn read(
        original: &'read ResourceDelta<'_>,
        targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        let source = match original {
            ResourceDelta::Consumption { source, .. }
            | ResourceDelta::Generation { source, .. }
            | ResourceDelta::Conversion { source, .. }
            | ResourceDelta::Storage { source, .. }
            | ResourceDelta::NetworkReflection { source, .. }
            | ResourceDelta::UnknownDelta { source } => source,
        };
        Ok(Self {
            source_type: super::text(&source.source_type)?,
            source_position: completeness::cell(PATH, &source.source_position)?,
            kind: Kind::read(original, targets)?,
        })
    }
}

impl<'read> Kind<'read> {
    fn read(
        original: &'read ResourceDelta<'_>,
        targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(match original {
            ResourceDelta::Consumption { fields, .. } => Self::Consumption {
                consumed: ResourceConsumption::read(fields, targets)?,
            },
            ResourceDelta::Generation {
                generation,
                composition,
                no_over_generation,
                ..
            } => Self::Generation {
                generation: amount(generation)?,
                composition: ratios(composition, targets)?,
                no_over_generation: completeness::cell(PATH, no_over_generation)?,
            },
            ResourceDelta::Conversion {
                consumed,
                generation,
                generated_composition,
                no_over_generation,
                ..
            } => Self::Conversion {
                consumed: ResourceConsumption::read(consumed, targets)?,
                generation: amount(generation)?,
                generated_composition: ratios(generated_composition, targets)?,
                no_over_generation: completeness::cell(PATH, no_over_generation)?,
            },
            ResourceDelta::Storage {
                consumption,
                generation,
                consumption_composition,
                discharge,
                ..
            } => Self::Storage {
                consumption: amount(consumption)?,
                generation: amount(generation)?,
                consumption_composition: ratios(consumption_composition, targets)?,
                discharge: completeness::cell(PATH, discharge)?,
            },
            ResourceDelta::NetworkReflection {
                resource,
                binary_evaluation,
                ..
            } => Self::NetworkReflection {
                resource: super::text(resource)?,
                binary_evaluation: completeness::cell(PATH, binary_evaluation)?,
            },
            ResourceDelta::UnknownDelta { .. } => Self::UnknownDelta,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResourceConsumption<'read> {
    minimum_consumption_fraction: Cell<Number<'static>>,
    consumption: Cell<Amount<'read>>,
    consumption_composition: Cell<Composition<'read>>,
}

impl<'read> ResourceConsumption<'read> {
    fn read(
        original: &'read ConsumptionFields<'_>,
        targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            minimum_consumption_fraction: super::single(&original.minimum_consumption_fraction)?,
            consumption: amount(&original.consumption)?,
            consumption_composition: ratios(&original.consumption_composition, targets)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Amount<'read> {
    resource: Cell<&'read str>,
    resource_amount_per_second: Cell<Unit<'read>>,
}

fn amount<'read>(
    original: &'read SourcedValue<ResourceDeltaAmount<'_>>,
) -> Result<Cell<Amount<'read>>, ReducedError> {
    completeness::cell(PATH, original)?.try_map(|rate| {
        Ok(Amount {
            resource: super::text(&rate.resource)?,
            resource_amount_per_second: completeness::cell(PATH, &rate.resource_amount_per_second)?
                .try_map(Unit::read)?,
        })
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Unit<'read> {
    source_type: Cell<&'read str>,
    #[serde(flatten)]
    amount: UnitAmount<'read>,
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum UnitAmount<'read> {
    Base,
    CentiResourceUnit {
        centi_resource_units: Cell<Number<'read>>,
    },
    MicroResourceUnit {
        micro_resource_units: Cell<Number<'read>>,
    },
    StandardResourceUnit {
        standard_resource_units: Cell<Number<'read>>,
    },
    PowerSegmentResourceUnit {
        units: Cell<Number<'read>>,
    },
    UnknownUnit,
}

impl<'read> Unit<'read> {
    fn read(original: &'read ResourceFlowUnit<'_>) -> Result<Self, ReducedError> {
        let (source, amount) = match original {
            ResourceFlowUnit::Base { source } => (source, UnitAmount::Base),
            ResourceFlowUnit::CentiResourceUnit {
                source,
                centi_resource_units,
            } => (
                source,
                UnitAmount::CentiResourceUnit {
                    centi_resource_units: number(centi_resource_units)?,
                },
            ),
            ResourceFlowUnit::MicroResourceUnit {
                source,
                micro_resource_units,
            } => (
                source,
                UnitAmount::MicroResourceUnit {
                    micro_resource_units: number(micro_resource_units)?,
                },
            ),
            ResourceFlowUnit::StandardResourceUnit {
                source,
                standard_resource_units,
            } => (
                source,
                UnitAmount::StandardResourceUnit {
                    standard_resource_units: number(standard_resource_units)?,
                },
            ),
            ResourceFlowUnit::PowerSegmentResourceUnit { source, units } => (
                source,
                UnitAmount::PowerSegmentResourceUnit {
                    units: number(units)?,
                },
            ),
            ResourceFlowUnit::UnknownUnit { source } => (source, UnitAmount::UnknownUnit),
        };
        Ok(Self {
            source_type: super::text(source.source_type())?,
            amount,
        })
    }
}

fn number(original: &SourcedValue<ResourceNumber>) -> Result<Cell<Number<'_>>, ReducedError> {
    Ok(completeness::cell(PATH, original)?.map(Number::from))
}

#[derive(Serialize)]
struct Composition<'read> {
    values: Cell<Vec<Cell<Ratio<'read>>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Ratio<'read> {
    container_resource: Cell<RecordLink<'read>>,
    ratio: Cell<Number<'static>>,
}

impl<'read> Ratio<'read> {
    fn read(
        original: &'read CompositionValue<'_>,
        targets: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            container_resource: completeness::cell(PATH, &original.container_resource)?
                .try_map(|link| RecordLink::read_source(PATH, link, targets, |_| true))?,
            ratio: super::single(&original.ratio)?,
        })
    }
}

fn ratios<'read>(
    original: &'read SourcedValue<ResourceComposition<'_>>,
    targets: &RecordTargets,
) -> Result<Cell<Composition<'read>>, ReducedError> {
    completeness::cell(PATH, original)?.try_map(|composition| {
        Ok(Composition {
            values: completeness::list(PATH, &composition.values, |entry| {
                Ratio::read(entry, targets)
            })?,
        })
    })
}
