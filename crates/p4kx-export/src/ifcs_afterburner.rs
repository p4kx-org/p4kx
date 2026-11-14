// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::ifcs_curve::IfcsCurve;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::inventory_dimensions::InventoryVector3;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct IfcsBurnTiming<'source> {
    #[serde(rename = "afterburnerAngCapacitorScaling")]
    ang_capacitor_scaling: SourcedValue<bool>,
    #[serde(rename = "afterburnerAngCapacitorScalingCurve")]
    ang_capacitor_scaling_curve: SourcedValue<IfcsCurve<'source>>,
    #[serde(rename = "afterburnerCapacitorThresholdRatio")]
    capacitor_threshold_ratio: SourcedValue<ResourceFloat32>,
    #[serde(rename = "afterburnerPreDelayTime")]
    pre_delay_time: SourcedValue<ResourceFloat32>,
    #[serde(rename = "afterburnerRampDownTime")]
    ramp_down_time: SourcedValue<ResourceFloat32>,
    #[serde(rename = "afterburnerRampMapping")]
    ramp_mapping: SourcedValue<IfcsCurve<'source>>,
    #[serde(rename = "afterburnerRampUpTime")]
    ramp_up_time: SourcedValue<ResourceFloat32>,
}

impl<'source> IfcsBurnTiming<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            ang_capacitor_scaling: fields.boolean("afterburnerAngCapacitorScaling", notes)?,
            ang_capacitor_scaling_curve: fields.declared_class(
                "afterburnerAngCapacitorScalingCurve",
                notes,
                IfcsCurve::read,
            )?,
            capacitor_threshold_ratio: fields
                .single("afterburnerCapacitorThresholdRatio", notes)?,
            pre_delay_time: fields.single("afterburnerPreDelayTime", notes)?,
            ramp_down_time: fields.single("afterburnerRampDownTime", notes)?,
            ramp_mapping: fields.declared_class(
                "afterburnerRampMapping",
                notes,
                IfcsCurve::read,
            )?,
            ramp_up_time: fields.single("afterburnerRampUpTime", notes)?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 7] {
        [
            (
                "afterburnerAngCapacitorScaling",
                self.ang_capacitor_scaling.state(),
            ),
            (
                "afterburnerAngCapacitorScalingCurve",
                self.ang_capacitor_scaling_curve.state(),
            ),
            (
                "afterburnerCapacitorThresholdRatio",
                self.capacitor_threshold_ratio.state(),
            ),
            ("afterburnerPreDelayTime", self.pre_delay_time.state()),
            ("afterburnerRampDownTime", self.ramp_down_time.state()),
            ("afterburnerRampMapping", self.ramp_mapping.state()),
            ("afterburnerRampUpTime", self.ramp_up_time.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct IfcsBurnResponse<'source> {
    #[serde(rename = "afterburnAccelMultiplierNegative")]
    accel_multiplier_negative: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnAccelMultiplierPositive")]
    accel_multiplier_positive: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnAngAccelMultiplier")]
    ang_accel_multiplier: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnAngTimeToFullAccelerationMultiplier")]
    ang_time_to_full_acceleration_multiplier: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnAngVelocityMultiplier")]
    ang_velocity_multiplier: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnLinSpeedMultiplierNegative")]
    lin_speed_multiplier_negative: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnLinSpeedMultiplierPositive")]
    lin_speed_multiplier_positive: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnLinTimeToFullAccelerationMultiplier")]
    lin_time_to_full_acceleration_multiplier: SourcedValue<InventoryVector3<'source>>,
}

impl<'source> IfcsBurnResponse<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            accel_multiplier_negative: fields.declared_class(
                "afterburnAccelMultiplierNegative",
                notes,
                InventoryVector3::read,
            )?,
            accel_multiplier_positive: fields.declared_class(
                "afterburnAccelMultiplierPositive",
                notes,
                InventoryVector3::read,
            )?,
            ang_accel_multiplier: fields.declared_class(
                "afterburnAngAccelMultiplier",
                notes,
                InventoryVector3::read,
            )?,
            ang_time_to_full_acceleration_multiplier: fields.declared_class(
                "afterburnAngTimeToFullAccelerationMultiplier",
                notes,
                InventoryVector3::read,
            )?,
            ang_velocity_multiplier: fields.declared_class(
                "afterburnAngVelocityMultiplier",
                notes,
                InventoryVector3::read,
            )?,
            lin_speed_multiplier_negative: fields.declared_class(
                "afterburnLinSpeedMultiplierNegative",
                notes,
                InventoryVector3::read,
            )?,
            lin_speed_multiplier_positive: fields.declared_class(
                "afterburnLinSpeedMultiplierPositive",
                notes,
                InventoryVector3::read,
            )?,
            lin_time_to_full_acceleration_multiplier: fields.declared_class(
                "afterburnLinTimeToFullAccelerationMultiplier",
                notes,
                InventoryVector3::read,
            )?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 8] {
        [
            (
                "afterburnAccelMultiplierNegative",
                self.accel_multiplier_negative.state(),
            ),
            (
                "afterburnAccelMultiplierPositive",
                self.accel_multiplier_positive.state(),
            ),
            (
                "afterburnAngAccelMultiplier",
                self.ang_accel_multiplier.state(),
            ),
            (
                "afterburnAngTimeToFullAccelerationMultiplier",
                self.ang_time_to_full_acceleration_multiplier.state(),
            ),
            (
                "afterburnAngVelocityMultiplier",
                self.ang_velocity_multiplier.state(),
            ),
            (
                "afterburnLinSpeedMultiplierNegative",
                self.lin_speed_multiplier_negative.state(),
            ),
            (
                "afterburnLinSpeedMultiplierPositive",
                self.lin_speed_multiplier_positive.state(),
            ),
            (
                "afterburnLinTimeToFullAccelerationMultiplier",
                self.lin_time_to_full_acceleration_multiplier.state(),
            ),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct IfcsBurnCapacitor<'source> {
    #[serde(rename = "capacitorAfterburnerAngularCost")]
    afterburner_angular_cost: SourcedValue<ResourceFloat32>,
    #[serde(rename = "capacitorAfterburnerIdleCost")]
    afterburner_idle_cost: SourcedValue<ResourceFloat32>,
    #[serde(rename = "capacitorAfterburnerLinearCost")]
    afterburner_linear_cost: SourcedValue<ResourceFloat32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "capacitorAssignmentInputOutputRegen")]
    assignment_input_output_regen: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "capacitorAssignmentInputOutputRegenNavMode")]
    assignment_input_output_regen_nav_mode: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "capacitorAssignmentInputOutputUsage")]
    assignment_input_output_usage: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[serde(rename = "capacitorMax")]
    max: SourcedValue<ResourceFloat32>,
    #[serde(rename = "capacitorRegenDelayAfterUse")]
    regen_delay_after_use: SourcedValue<ResourceFloat32>,
    #[serde(rename = "capacitorRegenPerSec")]
    regen_per_sec: SourcedValue<ResourceFloat32>,
}

impl<'source> IfcsBurnCapacitor<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            afterburner_angular_cost: fields.single("capacitorAfterburnerAngularCost", notes)?,
            afterburner_idle_cost: fields.single("capacitorAfterburnerIdleCost", notes)?,
            afterburner_linear_cost: fields.single("capacitorAfterburnerLinearCost", notes)?,
            assignment_input_output_regen: fields
                .record_link("capacitorAssignmentInputOutputRegen", notes)?,
            assignment_input_output_regen_nav_mode: fields
                .record_link("capacitorAssignmentInputOutputRegenNavMode", notes)?,
            assignment_input_output_usage: fields
                .record_link("capacitorAssignmentInputOutputUsage", notes)?,
            max: fields.single("capacitorMax", notes)?,
            regen_delay_after_use: fields.single("capacitorRegenDelayAfterUse", notes)?,
            regen_per_sec: fields.single("capacitorRegenPerSec", notes)?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 9] {
        [
            (
                "capacitorAfterburnerAngularCost",
                self.afterburner_angular_cost.state(),
            ),
            (
                "capacitorAfterburnerIdleCost",
                self.afterburner_idle_cost.state(),
            ),
            (
                "capacitorAfterburnerLinearCost",
                self.afterburner_linear_cost.state(),
            ),
            (
                "capacitorAssignmentInputOutputRegen",
                self.assignment_input_output_regen.state(),
            ),
            (
                "capacitorAssignmentInputOutputRegenNavMode",
                self.assignment_input_output_regen_nav_mode.state(),
            ),
            (
                "capacitorAssignmentInputOutputUsage",
                self.assignment_input_output_usage.state(),
            ),
            ("capacitorMax", self.max.state()),
            (
                "capacitorRegenDelayAfterUse",
                self.regen_delay_after_use.state(),
            ),
            ("capacitorRegenPerSec", self.regen_per_sec.state()),
        ]
    }
}

/// Independently authored response, timing and capacitor parameters of one afterburner block.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct IfcsAfterburner<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    timing: IfcsBurnTiming<'source>,
    #[serde(flatten)]
    response: IfcsBurnResponse<'source>,
    #[serde(flatten)]
    capacitor: IfcsBurnCapacitor<'source>,
}

impl<'source> IfcsAfterburner<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let timing = IfcsBurnTiming::read(fields, notes)?;
        let response = IfcsBurnResponse::read(fields, notes)?;
        let capacitor = IfcsBurnCapacitor::read(fields, notes)?;
        let source = UnitSource::read(
            fields,
            timing
                .projected()
                .into_iter()
                .chain(response.projected())
                .chain(capacitor.projected()),
        )?;
        Ok(Self {
            source,
            timing,
            response,
            capacitor,
        })
    }
}
