// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_v3::authored_effect_resource::Rtpc;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

/// One physical global configuration addressed by a mining laser's authored Reference.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MiningLaserGlobalDefinition<'source> {
    source: CanonicalRecordHeader<'source>,
    block_throttle_change_when_not_firing: SourcedValue<bool>,
    throttle_reset_on_stop_fire: SourcedValue<bool>,
    throttle_change_per_action: SourcedValue<ResourceFloat32>,
    throttle_acc_period: SourcedValue<ResourceFloat32>,
    throttle_acc_factor: SourcedValue<ResourceFloat32>,
    throttle_hold_acc_factor: SourcedValue<ResourceFloat32>,
    throttle_rtpc: SourcedValue<Rtpc>,
    mineable_glow_strength_rtpc: SourcedValue<Rtpc>,
    mineable_optimal_break_zone_rtpc: SourcedValue<Rtpc>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> MiningLaserGlobalDefinition<'source> {
    pub(super) fn read(
        _source_type: &'source str,
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let block_throttle_change_when_not_firing =
            fields.boolean("blockThrottleChangeWhenNotFiring", notes)?;
        let throttle_reset_on_stop_fire = fields.boolean("throttleResetOnStopFire", notes)?;
        let throttle_change_per_action = fields.single("throttleChangePerAction", notes)?;
        let throttle_acc_period = fields.single("throttleAccPeriod", notes)?;
        let throttle_acc_factor = fields.single("throttleAccFactor", notes)?;
        let throttle_hold_acc_factor = fields.single("throttleHoldAccFactor", notes)?;
        let throttle_rtpc =
            fields.declared_class_to("throttleRTPC", "AudioRtpc", notes, Rtpc::read)?;
        let mineable_glow_strength_rtpc =
            fields.declared_class_to("mineableGlowStrengthRTPC", "AudioRtpc", notes, Rtpc::read)?;
        let mineable_optimal_break_zone_rtpc = fields.declared_class_to(
            "mineableOptimalBreakZoneRTPC",
            "AudioRtpc",
            notes,
            Rtpc::read,
        )?;
        let additional_fields = fields.authored_members_except([
            (
                "blockThrottleChangeWhenNotFiring",
                block_throttle_change_when_not_firing.state(),
            ),
            (
                "throttleResetOnStopFire",
                throttle_reset_on_stop_fire.state(),
            ),
            (
                "throttleChangePerAction",
                throttle_change_per_action.state(),
            ),
            ("throttleAccPeriod", throttle_acc_period.state()),
            ("throttleAccFactor", throttle_acc_factor.state()),
            ("throttleHoldAccFactor", throttle_hold_acc_factor.state()),
            ("throttleRTPC", throttle_rtpc.state()),
            (
                "mineableGlowStrengthRTPC",
                mineable_glow_strength_rtpc.state(),
            ),
            (
                "mineableOptimalBreakZoneRTPC",
                mineable_optimal_break_zone_rtpc.state(),
            ),
        ])?;
        Ok(Self {
            source,
            block_throttle_change_when_not_firing,
            throttle_reset_on_stop_fire,
            throttle_change_per_action,
            throttle_acc_period,
            throttle_acc_factor,
            throttle_hold_acc_factor,
            throttle_rtpc,
            mineable_glow_strength_rtpc,
            mineable_optimal_break_zone_rtpc,
            additional_fields,
        })
    }
}
