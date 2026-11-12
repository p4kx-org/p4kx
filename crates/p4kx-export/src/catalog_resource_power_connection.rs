// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// Authored electrical connection parameters of an original component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ResourcePowerConnection<'source> {
    #[serde(flatten)]
    supply: PowerSupply,
    #[serde(flatten)]
    performance: PowerPerformance,
    #[serde(flatten)]
    presentation: PowerPresentation,
    #[serde(flatten)]
    misfires: PowerMisfires<'source>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourcePowerConnection<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let supply = PowerSupply::read(source, notes)?;
        let performance = PowerPerformance::read(source, notes)?;
        let presentation = PowerPresentation::read(source, notes)?;
        let misfires = PowerMisfires::read(source, notes)?;
        let additional_fields = source.authored_members_except(
            supply
                .projected()
                .into_iter()
                .chain(performance.projected())
                .chain(presentation.projected())
                .chain(misfires.projected()),
        )?;
        Ok(Self {
            supply,
            performance,
            presentation,
            misfires,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
#[schemars(transform = every_field_required)]
struct PowerSupply {
    power_base: SourcedValue<ResourceFloat32>,
    power_draw: SourcedValue<ResourceFloat32>,
    time_to_reach_draw_request: SourcedValue<ResourceFloat32>,
    safeguard_priority: SourcedValue<i32>,
    displayed_in_powered_item_list: SourcedValue<bool>,
    is_throttleable: SourcedValue<bool>,
}

impl PowerSupply {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            power_base: source.single("PowerBase", notes)?,
            power_draw: source.single("PowerDraw", notes)?,
            time_to_reach_draw_request: source.single("TimeToReachDrawRequest", notes)?,
            safeguard_priority: source.int32("SafeguardPriority", notes)?,
            displayed_in_powered_item_list: source.boolean("DisplayedInPoweredItemList", notes)?,
            is_throttleable: source.boolean("IsThrottleable", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 6] {
        [
            ("PowerBase", self.power_base.state()),
            ("PowerDraw", self.power_draw.state()),
            (
                "TimeToReachDrawRequest",
                self.time_to_reach_draw_request.state(),
            ),
            ("SafeguardPriority", self.safeguard_priority.state()),
            (
                "DisplayedInPoweredItemList",
                self.displayed_in_powered_item_list.state(),
            ),
            ("IsThrottleable", self.is_throttleable.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
#[schemars(transform = every_field_required)]
struct PowerPerformance {
    is_overclockable: SourcedValue<bool>,
    overclock_threshold_min: SourcedValue<ResourceFloat32>,
    overclock_threshold_max: SourcedValue<ResourceFloat32>,
    overpower_performance: SourcedValue<ResourceFloat32>,
    overclock_performance: SourcedValue<ResourceFloat32>,
}

impl PowerPerformance {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            is_overclockable: source.boolean("IsOverclockable", notes)?,
            overclock_threshold_min: source.single("OverclockThresholdMin", notes)?,
            overclock_threshold_max: source.single("OverclockThresholdMax", notes)?,
            overpower_performance: source.single("OverpowerPerformance", notes)?,
            overclock_performance: source.single("OverclockPerformance", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 5] {
        [
            ("IsOverclockable", self.is_overclockable.state()),
            (
                "OverclockThresholdMin",
                self.overclock_threshold_min.state(),
            ),
            (
                "OverclockThresholdMax",
                self.overclock_threshold_max.state(),
            ),
            ("OverpowerPerformance", self.overpower_performance.state()),
            ("OverclockPerformance", self.overclock_performance.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
#[schemars(transform = every_field_required)]
struct PowerPresentation {
    #[serde(rename = "PowerToEM")]
    power_to_em: SourcedValue<ResourceFloat32>,
    #[serde(rename = "DecayRateOfEM")]
    decay_rate_of_em: SourcedValue<ResourceFloat32>,
    warning_delay_time: SourcedValue<ResourceFloat32>,
    warning_display_time: SourcedValue<ResourceFloat32>,
}

impl PowerPresentation {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            power_to_em: source.single("PowerToEM", notes)?,
            decay_rate_of_em: source.single("DecayRateOfEM", notes)?,
            warning_delay_time: source.single("WarningDelayTime", notes)?,
            warning_display_time: source.single("WarningDisplayTime", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 4] {
        [
            ("PowerToEM", self.power_to_em.state()),
            ("DecayRateOfEM", self.decay_rate_of_em.state()),
            ("WarningDelayTime", self.warning_delay_time.state()),
            ("WarningDisplayTime", self.warning_display_time.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "PascalCase")]
#[schemars(transform = every_field_required)]
struct PowerMisfires<'source> {
    #[serde(rename = "MisfireItemTypeLocID")]
    item_type_loc_id: SourcedValue<SourceArenaOutput<'source>>,
    #[serde(rename = "MisfireGenerationParams")]
    generation_params: SourcedValue<SourceArenaOutput<'source>>,
    #[serde(rename = "MisfireEvents")]
    events: SourcedValue<SourceArenaOutput<'source>>,
}

impl<'source> PowerMisfires<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            item_type_loc_id: source.authored_member("MisfireItemTypeLocID", notes)?,
            generation_params: source.raw_strong_pointer_to(
                "MisfireGenerationParams",
                "SMisfireGenerationParams",
                notes,
            )?,
            events: source.authored_member("MisfireEvents", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 3] {
        [
            ("MisfireItemTypeLocID", self.item_type_loc_id.state()),
            ("MisfireGenerationParams", self.generation_params.state()),
            ("MisfireEvents", self.events.state()),
        ]
    }
}
