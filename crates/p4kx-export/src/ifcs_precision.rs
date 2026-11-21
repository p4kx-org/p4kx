// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::OrderedCurve;
use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::inventory_dimensions::InventoryVector3;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored ifcs limits fields and their source evidence.
#[non_exhaustive]
pub struct IfcsLimits<'source> {
    /// Original scm speed field.
    pub scm_speed: SourcedValue<ResourceFloat32>,
    /// Original max speed field.
    pub max_speed: SourcedValue<ResourceFloat32>,
    /// Original boost speed forward field.
    pub boost_speed_forward: SourcedValue<ResourceFloat32>,
    /// Original boost speed backward field.
    pub boost_speed_backward: SourcedValue<ResourceFloat32>,
    /// Original max angular velocity field.
    pub max_angular_velocity: SourcedValue<InventoryVector3<'source>>,
    /// Original negative angular scale field.
    pub negative_angular_scale: SourcedValue<InventoryVector3<'source>>,
    /// Original negative linear scale field.
    pub negative_linear_scale: SourcedValue<InventoryVector3<'source>>,
    /// Original positive angular scale field.
    pub positive_angular_scale: SourcedValue<InventoryVector3<'source>>,
    /// Original positive linear scale field.
    pub positive_linear_scale: SourcedValue<InventoryVector3<'source>>,
    /// Original linear limiter type field.
    pub linear_limiter_type: SourcedValue<&'source str>,
    /// Original pitch yaw limiter type field.
    pub pitch_yaw_limiter_type: SourcedValue<&'source str>,
    /// Original angular accel decay field.
    pub angular_accel_decay: SourcedValue<ResourceFloat32>,
    /// Original linear accel decay field.
    pub linear_accel_decay: SourcedValue<ResourceFloat32>,
    /// Original torque distance threshold field.
    pub torque_distance_threshold: SourcedValue<ResourceFloat32>,
    /// Original torque imbalance multiplier field.
    pub torque_imbalance_multiplier: SourcedValue<ResourceFloat32>,
}

impl<'source> IfcsLimits<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            scm_speed: fields.single("scmSpeed", notes)?,
            max_speed: fields.single("maxSpeed", notes)?,
            boost_speed_forward: fields.single("boostSpeedForward", notes)?,
            boost_speed_backward: fields.single("boostSpeedBackward", notes)?,
            max_angular_velocity: fields.declared_class(
                "maxAngularVelocity",
                notes,
                InventoryVector3::read,
            )?,
            negative_angular_scale: fields.declared_class(
                "negativeAngularScale",
                notes,
                InventoryVector3::read,
            )?,
            negative_linear_scale: fields.declared_class(
                "negativeLinearScale",
                notes,
                InventoryVector3::read,
            )?,
            positive_angular_scale: fields.declared_class(
                "positiveAngularScale",
                notes,
                InventoryVector3::read,
            )?,
            positive_linear_scale: fields.declared_class(
                "positiveLinearScale",
                notes,
                InventoryVector3::read,
            )?,
            linear_limiter_type: fields.borrowed_declared_enum("linearLimiterType", notes)?,
            pitch_yaw_limiter_type: fields.borrowed_declared_enum("pitchYawLimiterType", notes)?,
            angular_accel_decay: fields.single("angularAccelDecay", notes)?,
            linear_accel_decay: fields.single("linearAccelDecay", notes)?,
            torque_distance_threshold: fields.single("torqueDistanceThreshold", notes)?,
            torque_imbalance_multiplier: fields.single("torqueImbalanceMultiplier", notes)?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 15] {
        [
            ("scmSpeed", self.scm_speed.state()),
            ("maxSpeed", self.max_speed.state()),
            ("boostSpeedForward", self.boost_speed_forward.state()),
            ("boostSpeedBackward", self.boost_speed_backward.state()),
            ("maxAngularVelocity", self.max_angular_velocity.state()),
            ("negativeAngularScale", self.negative_angular_scale.state()),
            ("negativeLinearScale", self.negative_linear_scale.state()),
            ("positiveAngularScale", self.positive_angular_scale.state()),
            ("positiveLinearScale", self.positive_linear_scale.state()),
            ("linearLimiterType", self.linear_limiter_type.state()),
            ("pitchYawLimiterType", self.pitch_yaw_limiter_type.state()),
            ("angularAccelDecay", self.angular_accel_decay.state()),
            ("linearAccelDecay", self.linear_accel_decay.state()),
            (
                "torqueDistanceThreshold",
                self.torque_distance_threshold.state(),
            ),
            (
                "torqueImbalanceMultiplier",
                self.torque_imbalance_multiplier.state(),
            ),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored ifcs precision fields and their source evidence.
#[non_exhaustive]
pub struct IfcsPrecision<'source> {
    /// Original precision accel curve field.
    pub precision_accel_curve: SourcedValue<OrderedCurve<'source>>,
    /// Original precision landing multiplier field.
    pub precision_landing_multiplier: SourcedValue<ResourceFloat32>,
    /// Original precision max distance field.
    pub precision_max_distance: SourcedValue<ResourceFloat32>,
    /// Original precision min distance field.
    pub precision_min_distance: SourcedValue<ResourceFloat32>,
    /// Original max speed precision mode full proximity assist field.
    pub max_speed_precision_mode_full_proximity_assist: SourcedValue<ResourceFloat32>,
    /// Original max speed precision mode zero proximity assist field.
    pub max_speed_precision_mode_zero_proximity_assist: SourcedValue<ResourceFloat32>,
    /// Original refresh caches on landing mode field.
    pub refresh_caches_on_landing_mode: SourcedValue<bool>,
}

impl<'source> IfcsPrecision<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            precision_accel_curve: fields.declared_class(
                "precisionAccelCurve",
                notes,
                OrderedCurve::read,
            )?,
            precision_landing_multiplier: fields.single("precisionLandingMultiplier", notes)?,
            precision_max_distance: fields.single("precisionMaxDistance", notes)?,
            precision_min_distance: fields.single("precisionMinDistance", notes)?,
            max_speed_precision_mode_full_proximity_assist: fields
                .single("maxSpeedPrecisionModeFullProximityAssist", notes)?,
            max_speed_precision_mode_zero_proximity_assist: fields
                .single("maxSpeedPrecisionModeZeroProximityAssist", notes)?,
            refresh_caches_on_landing_mode: fields.boolean("refreshCachesOnLandingMode", notes)?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 7] {
        [
            ("precisionAccelCurve", self.precision_accel_curve.state()),
            (
                "precisionLandingMultiplier",
                self.precision_landing_multiplier.state(),
            ),
            ("precisionMaxDistance", self.precision_max_distance.state()),
            ("precisionMinDistance", self.precision_min_distance.state()),
            (
                "maxSpeedPrecisionModeFullProximityAssist",
                self.max_speed_precision_mode_full_proximity_assist.state(),
            ),
            (
                "maxSpeedPrecisionModeZeroProximityAssist",
                self.max_speed_precision_mode_zero_proximity_assist.state(),
            ),
            (
                "refreshCachesOnLandingMode",
                self.refresh_caches_on_landing_mode.state(),
            ),
        ]
    }
}
