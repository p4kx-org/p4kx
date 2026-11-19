// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::ifcs_afterburner::IfcsAfterburner;
use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::inventory_dimensions::InventoryVector3;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[path = "ifcs_speed_profile.rs"]
mod speed_profile;
pub use speed_profile::IfcsSpeedProfile;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored ifcs dynamics fields and their source evidence.
#[non_exhaustive]
pub struct IfcsDynamics<'source> {
    /// Original acceleration data field.
    pub acceleration_data: SourcedValue<IfcsAcceleration<'source>>,
    /// Original speed profile field.
    pub speed_profile: SourcedValue<IfcsSpeedProfile<'source>>,
    /// Original jerk profile field.
    pub jerk_profile: SourcedValue<IfcsJerk<'source>>,
    /// Original ifcs core params field.
    pub ifcs_core_params: SourcedValue<IfcsCore<'source>>,
    /// Original authored motion limits field.
    pub authored_motion_limits: SourcedValue<IfcsMotionLimits<'source>>,
    /// Original afterburner field.
    pub afterburner: SourcedValue<IfcsAfterburner<'source>>,
    /// Original afterburner new field.
    pub afterburner_new: SourcedValue<IfcsAfterburner<'source>>,
}

impl<'source> IfcsDynamics<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            acceleration_data: fields.declared_class(
                "accelerationData",
                notes,
                IfcsAcceleration::read,
            )?,
            speed_profile: fields.declared_class("speedProfile", notes, IfcsSpeedProfile::read)?,
            jerk_profile: fields.declared_class("jerkProfile", notes, IfcsJerk::read)?,
            ifcs_core_params: fields.declared_class("ifcsCoreParams", notes, IfcsCore::read)?,
            authored_motion_limits: fields.declared_class(
                "authoredMotionLimits",
                notes,
                IfcsMotionLimits::read,
            )?,
            afterburner: fields.declared_class("afterburner", notes, IfcsAfterburner::read)?,
            afterburner_new: fields.declared_class(
                "afterburnerNew",
                notes,
                IfcsAfterburner::read,
            )?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 7] {
        [
            ("accelerationData", self.acceleration_data.state()),
            ("speedProfile", self.speed_profile.state()),
            ("jerkProfile", self.jerk_profile.state()),
            ("ifcsCoreParams", self.ifcs_core_params.state()),
            ("authoredMotionLimits", self.authored_motion_limits.state()),
            ("afterburner", self.afterburner.state()),
            ("afterburnerNew", self.afterburner_new.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored ifcs acceleration fields and their source evidence.
#[non_exhaustive]
pub struct IfcsAcceleration<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    /// Original angular acceleration limiter field.
    pub angular_acceleration_limiter: SourcedValue<&'source str>,
    /// Original linear acceleration limiter default field.
    pub linear_acceleration_limiter_default: SourcedValue<&'source str>,
    /// Original angular thrust power assignment modifier field.
    pub angular_thrust_power_assignment_modifier: SourcedValue<SourceArenaOutput<'source>>,
    /// Original linear thrust power assignment modifier field.
    pub linear_thrust_power_assignment_modifier: SourcedValue<SourceArenaOutput<'source>>,
    /// Original negative angular scale field.
    pub negative_angular_scale: SourcedValue<InventoryVector3<'source>>,
    /// Original negative linear scale field.
    pub negative_linear_scale: SourcedValue<InventoryVector3<'source>>,
    /// Original positive angular scale field.
    pub positive_angular_scale: SourcedValue<InventoryVector3<'source>>,
    /// Original positive linear scale field.
    pub positive_linear_scale: SourcedValue<InventoryVector3<'source>>,
}

impl<'source> IfcsAcceleration<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let angular_acceleration_limiter =
            fields.borrowed_declared_enum("angularAccelerationLimiter", notes)?;
        let linear_acceleration_limiter_default =
            fields.borrowed_declared_enum("linearAccelerationLimiterDefault", notes)?;
        let angular_thrust_power_assignment_modifier =
            fields.authored_member("angularThrustPowerAssignmentModifier", notes)?;
        let linear_thrust_power_assignment_modifier =
            fields.authored_member("linearThrustPowerAssignmentModifier", notes)?;
        let negative_angular_scale =
            fields.declared_class("negativeAngularScale", notes, InventoryVector3::read)?;
        let negative_linear_scale =
            fields.declared_class("negativeLinearScale", notes, InventoryVector3::read)?;
        let positive_angular_scale =
            fields.declared_class("positiveAngularScale", notes, InventoryVector3::read)?;
        let positive_linear_scale =
            fields.declared_class("positiveLinearScale", notes, InventoryVector3::read)?;
        let source = UnitSource::read(
            fields,
            [
                (
                    "angularAccelerationLimiter",
                    angular_acceleration_limiter.state(),
                ),
                (
                    "linearAccelerationLimiterDefault",
                    linear_acceleration_limiter_default.state(),
                ),
                (
                    "angularThrustPowerAssignmentModifier",
                    angular_thrust_power_assignment_modifier.state(),
                ),
                (
                    "linearThrustPowerAssignmentModifier",
                    linear_thrust_power_assignment_modifier.state(),
                ),
                ("negativeAngularScale", negative_angular_scale.state()),
                ("negativeLinearScale", negative_linear_scale.state()),
                ("positiveAngularScale", positive_angular_scale.state()),
                ("positiveLinearScale", positive_linear_scale.state()),
            ],
        )?;
        Ok(Self {
            source,
            angular_acceleration_limiter,
            linear_acceleration_limiter_default,
            angular_thrust_power_assignment_modifier,
            linear_thrust_power_assignment_modifier,
            negative_angular_scale,
            negative_linear_scale,
            positive_angular_scale,
            positive_linear_scale,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored ifcs jerk fields and their source evidence.
#[non_exhaustive]
pub struct IfcsJerk<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    /// Original ang acceleration decay field.
    pub ang_acceleration_decay: SourcedValue<ResourceFloat32>,
    /// Original ang jerk time field.
    pub ang_jerk_time: SourcedValue<InventoryVector3<'source>>,
    /// Original lin acceleration decay field.
    pub lin_acceleration_decay: SourcedValue<ResourceFloat32>,
    /// Original lin jerk time field.
    pub lin_jerk_time: SourcedValue<InventoryVector3<'source>>,
}

impl<'source> IfcsJerk<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let ang_acceleration_decay = fields.single("angAccelerationDecay", notes)?;
        let ang_jerk_time = fields.declared_class("angJerkTime", notes, InventoryVector3::read)?;
        let lin_acceleration_decay = fields.single("linAccelerationDecay", notes)?;
        let lin_jerk_time = fields.declared_class("linJerkTime", notes, InventoryVector3::read)?;
        let source = UnitSource::read(
            fields,
            [
                ("angAccelerationDecay", ang_acceleration_decay.state()),
                ("angJerkTime", ang_jerk_time.state()),
                ("linAccelerationDecay", lin_acceleration_decay.state()),
                ("linJerkTime", lin_jerk_time.state()),
            ],
        )?;
        Ok(Self {
            source,
            ang_acceleration_decay,
            ang_jerk_time,
            lin_acceleration_decay,
            lin_jerk_time,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored ifcs core fields and their source evidence.
#[non_exhaustive]
pub struct IfcsCore<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    /// Original boot wait time field.
    pub boot_wait_time: SourcedValue<ResourceFloat32>,
    /// Original spool up time field.
    pub spool_up_time: SourcedValue<ResourceFloat32>,
    /// Original stun threshold gs field.
    pub stun_threshold_gs: SourcedValue<ResourceFloat32>,
}

impl<'source> IfcsCore<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let boot_wait_time = fields.single("bootWaitTime", notes)?;
        let spool_up_time = fields.single("spoolUpTime", notes)?;
        let stun_threshold_gs = fields.single("stunThresholdGs", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("bootWaitTime", boot_wait_time.state()),
                ("spoolUpTime", spool_up_time.state()),
                ("stunThresholdGs", stun_threshold_gs.state()),
            ],
        )?;
        Ok(Self {
            source,
            boot_wait_time,
            spool_up_time,
            stun_threshold_gs,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored ifcs motion limits fields and their source evidence.
#[non_exhaustive]
pub struct IfcsMotionLimits<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    /// Original linear max acceleration field.
    pub linear_max_acceleration: SourcedValue<ResourceFloat32>,
    /// Original rotation max acceleration field.
    pub rotation_max_acceleration: SourcedValue<InventoryVector3<'source>>,
}

impl<'source> IfcsMotionLimits<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let linear_max_acceleration = fields.single("linearMaxAcceleration", notes)?;
        let rotation_max_acceleration =
            fields.declared_class("rotationMaxAcceleration", notes, InventoryVector3::read)?;
        let source = UnitSource::read(
            fields,
            [
                ("linearMaxAcceleration", linear_max_acceleration.state()),
                ("rotationMaxAcceleration", rotation_max_acceleration.state()),
            ],
        )?;
        Ok(Self {
            source,
            linear_max_acceleration,
            rotation_max_acceleration,
        })
    }
}
