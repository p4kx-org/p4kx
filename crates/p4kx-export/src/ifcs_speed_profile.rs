// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::inventory_dimensions::InventoryVector3;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored ifcs speed profile fields and their source evidence.
#[non_exhaustive]
pub struct IfcsSpeedProfile<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    /// Original angular velocity field.
    pub angular_velocity: SourcedValue<InventoryVector3<'source>>,
    /// Original angular velocity limiter field.
    pub angular_velocity_limiter: SourcedValue<&'source str>,
    /// Original core off rotation modifier field.
    pub core_off_rotation_modifier: SourcedValue<SourceArenaOutput<'source>>,
    /// Original linear acceleration rotation modifier field.
    pub linear_acceleration_rotation_modifier: SourcedValue<SourceArenaOutput<'source>>,
    /// Original linear speed rotation modifier field.
    pub linear_speed_rotation_modifier: SourcedValue<SourceArenaOutput<'source>>,
    /// Original linear velocity limiter field.
    pub linear_velocity_limiter: SourcedValue<&'source str>,
    /// Original negative linear time to full speed field.
    pub negative_linear_time_to_full_speed: SourcedValue<InventoryVector3<'source>>,
    /// Original positive linear time to full speed field.
    pub positive_linear_time_to_full_speed: SourcedValue<InventoryVector3<'source>>,
    /// Original power assignment rotation modifier field.
    pub power_assignment_rotation_modifier: SourcedValue<SourceArenaOutput<'source>>,
    /// Original proximity assist field.
    pub proximity_assist: SourcedValue<SourceArenaOutput<'source>>,
}

impl<'source> IfcsSpeedProfile<'source> {
    #[expect(
        clippy::too_many_lines,
        reason = "one read and source-state entry per original field"
    )]
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let angular_velocity =
            fields.declared_class("angularVelocity", notes, InventoryVector3::read)?;
        let angular_velocity_limiter =
            fields.borrowed_declared_enum("angularVelocityLimiter", notes)?;
        let core_off_rotation_modifier =
            fields.authored_member("coreOffRotationModifier", notes)?;
        let linear_acceleration_rotation_modifier =
            fields.authored_member("linearAccelerationRotationModifier", notes)?;
        let linear_speed_rotation_modifier =
            fields.authored_member("linearSpeedRotationModifier", notes)?;
        let linear_velocity_limiter =
            fields.borrowed_declared_enum("linearVelocityLimiter", notes)?;
        let negative_linear_time_to_full_speed = fields.declared_class(
            "negativeLinearTimeToFullSpeed",
            notes,
            InventoryVector3::read,
        )?;
        let positive_linear_time_to_full_speed = fields.declared_class(
            "positiveLinearTimeToFullSpeed",
            notes,
            InventoryVector3::read,
        )?;
        let power_assignment_rotation_modifier =
            fields.authored_member("powerAssignmentRotationModifier", notes)?;
        let proximity_assist = fields.authored_member("proximityAssist", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("angularVelocity", angular_velocity.state()),
                ("angularVelocityLimiter", angular_velocity_limiter.state()),
                (
                    "coreOffRotationModifier",
                    core_off_rotation_modifier.state(),
                ),
                (
                    "linearAccelerationRotationModifier",
                    linear_acceleration_rotation_modifier.state(),
                ),
                (
                    "linearSpeedRotationModifier",
                    linear_speed_rotation_modifier.state(),
                ),
                ("linearVelocityLimiter", linear_velocity_limiter.state()),
                (
                    "negativeLinearTimeToFullSpeed",
                    negative_linear_time_to_full_speed.state(),
                ),
                (
                    "positiveLinearTimeToFullSpeed",
                    positive_linear_time_to_full_speed.state(),
                ),
                (
                    "powerAssignmentRotationModifier",
                    power_assignment_rotation_modifier.state(),
                ),
                ("proximityAssist", proximity_assist.state()),
            ],
        )?;
        Ok(Self {
            source,
            angular_velocity,
            angular_velocity_limiter,
            core_off_rotation_modifier,
            linear_acceleration_rotation_modifier,
            linear_speed_rotation_modifier,
            linear_velocity_limiter,
            negative_linear_time_to_full_speed,
            positive_linear_time_to_full_speed,
            power_assignment_rotation_modifier,
            proximity_assist,
        })
    }
}
