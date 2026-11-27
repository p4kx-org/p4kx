// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use super::resource_quality::{self, Curve, Overrides, Quality, Quantization};
use crate::resource_property_contract::{ResourceProperty, ResourcePropertyPayload};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Property<'source> {
    source_type: Cell<&'source str>,
    name: Cell<&'source str>,
    payload: Cell<Payload<'source>>,
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Payload<'source> {
    Base,
    Volatility {
        volatility: Cell<Number<'source>>,
        health_decay_per_second: Cell<Number<'source>>,
    },
    CraftingData(Box<CraftingData<'source>>),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CraftingData<'source> {
    #[serde(rename = "qualityDistribution")]
    distribution: Cell<Quality<'source, Curve<'source>>>,
    #[serde(rename = "qualityLocationOverride")]
    location_override: Cell<Quality<'source, Overrides<'source>>>,
    #[serde(rename = "qualityQuantization")]
    quantization: Cell<Quality<'source, Quantization<'source>>>,
}

pub(super) fn read<'source>(
    path: &'static str,
    value: &'source SourcedValue<Vec<SourcedValue<ResourceProperty<'_>>>>,
) -> Result<Cell<Vec<Cell<Property<'source>>>>, ReducedError> {
    completeness::list(path, value, |property| {
        let payload = completeness::cell(path, property.payload())?.try_map(|payload| {
            Ok::<_, ReducedError>(match payload {
                ResourcePropertyPayload::Base => Payload::Base,
                ResourcePropertyPayload::Volatility(source) => Payload::Volatility {
                    volatility: completeness::cell(path, source.volatility())?.map(Number::from),
                    health_decay_per_second: completeness::cell(
                        path,
                        source.health_decay_per_second(),
                    )?
                    .map(Number::from),
                },
                ResourcePropertyPayload::CraftingData(source) => {
                    Payload::CraftingData(Box::new(CraftingData {
                        distribution: resource_quality::distribution(
                            path,
                            source.quality_distribution(),
                        )?,
                        location_override: resource_quality::location_overrides(
                            path,
                            source.quality_location_override(),
                        )?,
                        quantization: resource_quality::quantization(
                            path,
                            source.quality_quantization(),
                        )?,
                    }))
                }
            })
        })?;
        Ok(Property {
            source_type: completeness::cell(path, property.source_type())?.map(|name| *name),
            name: completeness::cell(path, property.name())?.map(|name| *name),
            payload,
        })
    })
}
