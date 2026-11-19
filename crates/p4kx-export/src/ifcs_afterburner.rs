// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::OrderedCurve;
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
/// Original authored ifcs burn timing fields and their source evidence.
#[non_exhaustive]
pub struct IfcsBurnTiming<'source> {
    #[serde(rename = "afterburnerAngCapacitorScaling")]
    /// Original ang capacitor scaling field.
    pub ang_capacitor_scaling: SourcedValue<bool>,
    #[serde(rename = "afterburnerAngCapacitorScalingCurve")]
    /// Original ang capacitor scaling curve field.
    pub ang_capacitor_scaling_curve: SourcedValue<OrderedCurve<'source>>,
    #[serde(rename = "afterburnerCapacitorThresholdRatio")]
    /// Original capacitor threshold ratio field.
    pub capacitor_threshold_ratio: SourcedValue<ResourceFloat32>,
    #[serde(rename = "afterburnerPreDelayTime")]
    /// Original pre delay time field.
    pub pre_delay_time: SourcedValue<ResourceFloat32>,
    #[serde(rename = "afterburnerRampDownTime")]
    /// Original ramp down time field.
    pub ramp_down_time: SourcedValue<ResourceFloat32>,
    #[serde(rename = "afterburnerRampMapping")]
    /// Original ramp mapping field.
    pub ramp_mapping: SourcedValue<OrderedCurve<'source>>,
    #[serde(rename = "afterburnerRampUpTime")]
    /// Original ramp up time field.
    pub ramp_up_time: SourcedValue<ResourceFloat32>,
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
                OrderedCurve::read,
            )?,
            capacitor_threshold_ratio: fields
                .single("afterburnerCapacitorThresholdRatio", notes)?,
            pre_delay_time: fields.single("afterburnerPreDelayTime", notes)?,
            ramp_down_time: fields.single("afterburnerRampDownTime", notes)?,
            ramp_mapping: fields.declared_class(
                "afterburnerRampMapping",
                notes,
                OrderedCurve::read,
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
/// Original authored ifcs burn response fields and their source evidence.
#[non_exhaustive]
pub struct IfcsBurnResponse<'source> {
    #[serde(rename = "afterburnAccelMultiplierNegative")]
    /// Original accel multiplier negative field.
    pub accel_multiplier_negative: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnAccelMultiplierPositive")]
    /// Original accel multiplier positive field.
    pub accel_multiplier_positive: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnAngAccelMultiplier")]
    /// Original ang accel multiplier field.
    pub ang_accel_multiplier: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnAngTimeToFullAccelerationMultiplier")]
    /// Original ang time to full acceleration multiplier field.
    pub ang_time_to_full_acceleration_multiplier: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnAngVelocityMultiplier")]
    /// Original ang velocity multiplier field.
    pub ang_velocity_multiplier: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnLinSpeedMultiplierNegative")]
    /// Original lin speed multiplier negative field.
    pub lin_speed_multiplier_negative: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnLinSpeedMultiplierPositive")]
    /// Original lin speed multiplier positive field.
    pub lin_speed_multiplier_positive: SourcedValue<InventoryVector3<'source>>,
    #[serde(rename = "afterburnLinTimeToFullAccelerationMultiplier")]
    /// Original lin time to full acceleration multiplier field.
    pub lin_time_to_full_acceleration_multiplier: SourcedValue<InventoryVector3<'source>>,
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
/// Original authored ifcs burn capacitor fields and their source evidence.
#[non_exhaustive]
pub struct IfcsBurnCapacitor<'source> {
    #[serde(rename = "capacitorAfterburnerAngularCost")]
    /// Original afterburner angular cost field.
    pub afterburner_angular_cost: SourcedValue<ResourceFloat32>,
    #[serde(rename = "capacitorAfterburnerIdleCost")]
    /// Original afterburner idle cost field.
    pub afterburner_idle_cost: SourcedValue<ResourceFloat32>,
    #[serde(rename = "capacitorAfterburnerLinearCost")]
    /// Original afterburner linear cost field.
    pub afterburner_linear_cost: SourcedValue<ResourceFloat32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "capacitorAssignmentInputOutputRegen")]
    /// Original assignment input output regen field.
    pub assignment_input_output_regen: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "capacitorAssignmentInputOutputRegenNavMode")]
    /// Original assignment input output regen nav mode field.
    pub assignment_input_output_regen_nav_mode: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "capacitorAssignmentInputOutputUsage")]
    /// Original assignment input output usage field.
    pub assignment_input_output_usage: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[serde(rename = "capacitorMax")]
    /// Original max field.
    pub max: SourcedValue<ResourceFloat32>,
    #[serde(rename = "capacitorRegenDelayAfterUse")]
    /// Original regen delay after use field.
    pub regen_delay_after_use: SourcedValue<ResourceFloat32>,
    #[serde(rename = "capacitorRegenPerSec")]
    /// Original regen per sec field.
    pub regen_per_sec: SourcedValue<ResourceFloat32>,
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
#[non_exhaustive]
pub struct IfcsAfterburner<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    #[serde(flatten)]
    /// Original timing field.
    pub timing: IfcsBurnTiming<'source>,
    #[serde(flatten)]
    /// Original response field.
    pub response: IfcsBurnResponse<'source>,
    #[serde(flatten)]
    /// Original capacitor field.
    pub capacitor: IfcsBurnCapacitor<'source>,
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
