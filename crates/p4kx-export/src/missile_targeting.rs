// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// The stored targeting limits and original tracking-signal label.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MissileTargeting<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    tracking_signal_type: SourcedValue<&'source str>,
    tracking_signal_min: SourcedValue<ResourceFloat32>,
    lock_signal_amplifier: SourcedValue<ResourceFloat32>,
    lock_time: SourcedValue<ResourceFloat32>,
    locking_angle: SourcedValue<ResourceFloat32>,
    min_ratio_for_lock: SourcedValue<ResourceFloat32>,
    lock_increase_rate: SourcedValue<ResourceFloat32>,
    lock_decrease_rate: SourcedValue<ResourceFloat32>,
    lock_range_min: SourcedValue<ResourceFloat32>,
    lock_range_max: SourcedValue<ResourceFloat32>,
    lock_resolution_radius: SourcedValue<ResourceFloat32>,
    lock_resolution_falloff_power: SourcedValue<ResourceFloat32>,
    signal_resilience_min: SourcedValue<ResourceFloat32>,
    signal_resilience_max: SourcedValue<ResourceFloat32>,
    allow_dumb_firing: SourcedValue<bool>,
    max_times_can_miss: SourcedValue<i32>,
}

impl<'source> MissileTargeting<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tracking_signal_type =
            source.borrowed_enum_to("trackingSignalType", "ESignatureType", notes)?;
        let tracking_signal_min = source.single("trackingSignalMin", notes)?;
        let lock_signal_amplifier = source.single("lockSignalAmplifier", notes)?;
        let lock_time = source.single("lockTime", notes)?;
        let locking_angle = source.single("lockingAngle", notes)?;
        let min_ratio_for_lock = source.single("minRatioForLock", notes)?;
        let lock_increase_rate = source.single("lockIncreaseRate", notes)?;
        let lock_decrease_rate = source.single("lockDecreaseRate", notes)?;
        let lock_range_min = source.single("lockRangeMin", notes)?;
        let lock_range_max = source.single("lockRangeMax", notes)?;
        let lock_resolution_radius = source.single("lockResolutionRadius", notes)?;
        let lock_resolution_falloff_power = source.single("lockResolutionFalloffPower", notes)?;
        let signal_resilience_min = source.single("signalResilienceMin", notes)?;
        let signal_resilience_max = source.single("signalResilienceMax", notes)?;
        let allow_dumb_firing = source.boolean("allowDumbFiring", notes)?;
        let max_times_can_miss = source.int32("maxTimesCanMiss", notes)?;
        let source = UnitSource::read(
            source,
            [
                ("trackingSignalType", tracking_signal_type.state()),
                ("trackingSignalMin", tracking_signal_min.state()),
                ("lockSignalAmplifier", lock_signal_amplifier.state()),
                ("lockTime", lock_time.state()),
                ("lockingAngle", locking_angle.state()),
                ("minRatioForLock", min_ratio_for_lock.state()),
                ("lockIncreaseRate", lock_increase_rate.state()),
                ("lockDecreaseRate", lock_decrease_rate.state()),
                ("lockRangeMin", lock_range_min.state()),
                ("lockRangeMax", lock_range_max.state()),
                ("lockResolutionRadius", lock_resolution_radius.state()),
                (
                    "lockResolutionFalloffPower",
                    lock_resolution_falloff_power.state(),
                ),
                ("signalResilienceMin", signal_resilience_min.state()),
                ("signalResilienceMax", signal_resilience_max.state()),
                ("allowDumbFiring", allow_dumb_firing.state()),
                ("maxTimesCanMiss", max_times_can_miss.state()),
            ],
        )?;
        Ok(Self {
            source,
            tracking_signal_type,
            tracking_signal_min,
            lock_signal_amplifier,
            lock_time,
            locking_angle,
            min_ratio_for_lock,
            lock_increase_rate,
            lock_decrease_rate,
            lock_range_min,
            lock_range_max,
            lock_resolution_radius,
            lock_resolution_falloff_power,
            signal_resilience_min,
            signal_resilience_max,
            allow_dumb_firing,
            max_times_can_miss,
        })
    }
}
