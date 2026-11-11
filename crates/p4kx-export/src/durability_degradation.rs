// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::observed::Observed;
use super::wear::WearFields;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const ACCUMULATORS: &[&str] = &[
    "SAccumulatorParams",
    "SWearAccumulatorParams",
    "SDirtAccumulatorParams",
    "SWetnessAccumulatorParams",
];

/// The stored stop flag and ordered accumulators of one degradation component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct DegradationFields {
    stop_degrading_if_destroyed: SourcedValue<bool>,
    accumulators: SourcedValue<Vec<SourcedValue<Accumulator>>>,
}

impl DegradationFields {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            stop_degrading_if_destroyed: source.boolean("StopDegradingIfDestroyed", notes)?,
            accumulators: source.array("accumulators", ACCUMULATORS, notes, Accumulator::read)?,
        })
    }
}

/// Shared inherited controls and the actual accumulator's concrete branch.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Accumulator {
    common: AccumulatorBase,
    kind: AccumulatorKind,
}

impl Accumulator {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let common = AccumulatorBase::read(source, notes)?;
        let kind = match source.source_type()? {
            "SAccumulatorParams" => AccumulatorKind::Base,
            "SWearAccumulatorParams" => AccumulatorKind::Wear {
                wear: Box::new(WearFields::read(source, notes)?),
            },
            "SDirtAccumulatorParams" => AccumulatorKind::Dirt {
                initial_dirt_ratio: source.single("InitialDirtRatio", notes)?,
            },
            "SWetnessAccumulatorParams" => AccumulatorKind::Wetness,
            _ => return Err(crate::ExportAccountingError::ConflictingSourceDatum.into()),
        };
        Ok(Self { common, kind })
    }
}

/// Type-specific source fields of a known degradation accumulator.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum AccumulatorKind {
    Base,
    Wear {
        wear: Box<WearFields>,
    },
    Dirt {
        initial_dirt_ratio: SourcedValue<ResourceFloat32>,
    },
    Wetness,
}

/// Original inherited fields shared by accumulator variants.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct AccumulatorBase {
    initial_accumulation_ratio: SourcedValue<ResourceFloat32>,
    accumulate_when_unstreamed: SourcedValue<bool>,
    accumulate_only_after_tractor_beam: SourcedValue<bool>,
    accumulate_only_when_attached: SourcedValue<bool>,
    stop_accumulation_when_attached: SourcedValue<bool>,
    accumulation_event_threshold: SourcedValue<ResourceFloat32>,
    port_tags: SourcedValue<String>,
    required_port_tags: SourcedValue<String>,
}

impl AccumulatorBase {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            initial_accumulation_ratio: source.single("InitialAccumulationRatio", notes)?,
            accumulate_when_unstreamed: source.boolean("AccumulateWhenUnstreamed", notes)?,
            accumulate_only_after_tractor_beam: source
                .boolean("AccumulateOnlyAfterTractorBeam", notes)?,
            accumulate_only_when_attached: source.boolean("AccumulateOnlyWhenAttached", notes)?,
            stop_accumulation_when_attached: source
                .boolean("StopAccumulationWhenAttached", notes)?,
            accumulation_event_threshold: source.single("AccumulationEventThreshold", notes)?,
            port_tags: source.string("PortTags", notes)?,
            required_port_tags: source.string("RequiredPortTags", notes)?,
        })
    }
}
