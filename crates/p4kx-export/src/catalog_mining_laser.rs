// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use super::modifiers::{MiningFilter, MiningModifiers};
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

pub(super) const COMPONENT: &str = "SEntityComponentMiningLaserParams";
pub(super) const GLOBAL: &str = "MiningLaserGlobalParams";

/// Original throttle, filtering and modifiers on one physical mining laser component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MiningLaser<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    throttle_lerp_speed: SourcedValue<ResourceFloat32>,
    throttle_minimum: SourcedValue<ResourceFloat32>,
    uses_power_throttle: SourcedValue<bool>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    global_params: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[serde(rename = "miningLaserModifiers")]
    modifiers: SourcedValue<MiningModifiers<'source>>,
    filter_params: SourcedValue<MiningFilter<'source>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> MiningLaser<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let throttle_lerp_speed = source.single("throttleLerpSpeed", notes)?;
        let throttle_minimum = source.single("throttleMinimum", notes)?;
        let uses_power_throttle = source.boolean("usesPowerThrottle", notes)?;
        let global_params = source.record_link_to("globalParams", GLOBAL, notes)?;
        let modifiers = source.declared_class_to(
            "miningLaserModifiers",
            "MiningLaserModifiers",
            notes,
            MiningModifiers::read,
        )?;
        let filter_params = source.declared_class_to(
            "filterParams",
            "MiningFilterParams",
            notes,
            MiningFilter::read,
        )?;
        let original = UnitSource::read(
            source,
            [
                ("throttleLerpSpeed", throttle_lerp_speed.state()),
                ("throttleMinimum", throttle_minimum.state()),
                ("usesPowerThrottle", uses_power_throttle.state()),
                ("globalParams", global_params.state()),
                ("miningLaserModifiers", modifiers.state()),
                ("filterParams", filter_params.state()),
            ],
        )?;
        Ok(Self {
            owner,
            throttle_lerp_speed,
            throttle_minimum,
            uses_power_throttle,
            global_params,
            modifiers,
            filter_params,
            original,
        })
    }

    pub(super) fn global_index(&self) -> Option<u32> {
        self.global_params.value()?.resolved_index()
    }
}
