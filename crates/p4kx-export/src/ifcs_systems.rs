// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::OrderedCurve;
use super::ifcs_fuel::{IfcsFuelUsage, IfcsNoFuel, IfcsThrusterEfficiency};
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::inventory_dimensions::InventoryVector3;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored ifcs systems fields and their source evidence.
#[non_exhaustive]
pub struct IfcsSystems<'source> {
    /// Original formation mode params field.
    pub formation_mode_params: SourcedValue<SourceArenaOutput<'source>>,
    /// Original hud messages field.
    pub hud_messages: SourcedValue<SourceArenaOutput<'source>>,
    /// Original thruster imbalance message field.
    pub thruster_imbalance_message: SourcedValue<SourceArenaOutput<'source>>,
    /// Original thruster types for angular control field.
    pub thruster_types_for_angular_control: SourcedValue<SourceArenaOutput<'source>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// Original modifiers field.
    pub modifiers: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// Original legacy modifiers legacy field.
    pub modifiers_legacy: SourcedValue<Box<SourceRecordReference<'source>>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// Original intoxication modifier ref field.
    pub intoxication_modifier_ref: SourcedValue<Box<SourceRecordReference<'source>>>,
    /// Original fuel usage params field.
    pub fuel_usage_params: SourcedValue<IfcsFuelUsage<'source>>,
    /// Original thruster efficiency params field.
    pub thruster_efficiency_params: SourcedValue<IfcsThrusterEfficiency<'source>>,
    /// Original legacy thruster efficiency params field.
    pub legacy_thruster_efficiency_params: SourcedValue<IfcsThrusterEfficiency<'source>>,
    /// Original no fuel params field.
    pub no_fuel_params: SourcedValue<IfcsNoFuel<'source>>,
    /// Original legacy no fuel params legacy field.
    pub no_fuel_params_legacy: SourcedValue<IfcsNoFuel<'source>>,
    /// Original damage handling field.
    pub damage_handling: SourcedValue<IfcsDamageHandling<'source>>,
    /// Original part damage params field.
    pub part_damage_params: SourcedValue<IfcsPartDamage<'source>>,
}

impl<'source> IfcsSystems<'source> {
    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            formation_mode_params: fields.authored_member("formationModeParams", notes)?,
            hud_messages: fields.authored_member("hudMessages", notes)?,
            thruster_imbalance_message: fields
                .authored_member("thrusterImbalanceMessage", notes)?,
            thruster_types_for_angular_control: fields
                .authored_member("thrusterTypesForAngularControl", notes)?,
            modifiers: fields.record_link("modifiers", notes)?,
            modifiers_legacy: fields.record_link("modifiersLegacy", notes)?,
            intoxication_modifier_ref: fields.record_link("intoxicationModifierRef", notes)?,
            fuel_usage_params: fields.declared_class(
                "fuelUsageParams",
                notes,
                IfcsFuelUsage::read,
            )?,
            thruster_efficiency_params: fields.declared_class(
                "thrusterEfficiencyParams",
                notes,
                IfcsThrusterEfficiency::read,
            )?,
            legacy_thruster_efficiency_params: fields.declared_class(
                "legacyThrusterEfficiencyParams",
                notes,
                IfcsThrusterEfficiency::read,
            )?,
            no_fuel_params: fields.declared_class("noFuelParams", notes, IfcsNoFuel::read)?,
            no_fuel_params_legacy: fields.declared_class(
                "noFuelParamsLegacy",
                notes,
                IfcsNoFuel::read,
            )?,
            damage_handling: fields.declared_class(
                "damageHandling",
                notes,
                IfcsDamageHandling::read,
            )?,
            part_damage_params: fields.declared_class(
                "partDamageParams",
                notes,
                IfcsPartDamage::read,
            )?,
        })
    }

    pub(super) const fn projected(&self) -> [(&'static str, SourceValueState); 14] {
        [
            ("formationModeParams", self.formation_mode_params.state()),
            ("hudMessages", self.hud_messages.state()),
            (
                "thrusterImbalanceMessage",
                self.thruster_imbalance_message.state(),
            ),
            (
                "thrusterTypesForAngularControl",
                self.thruster_types_for_angular_control.state(),
            ),
            ("modifiers", self.modifiers.state()),
            ("modifiersLegacy", self.modifiers_legacy.state()),
            (
                "intoxicationModifierRef",
                self.intoxication_modifier_ref.state(),
            ),
            ("fuelUsageParams", self.fuel_usage_params.state()),
            (
                "thrusterEfficiencyParams",
                self.thruster_efficiency_params.state(),
            ),
            (
                "legacyThrusterEfficiencyParams",
                self.legacy_thruster_efficiency_params.state(),
            ),
            ("noFuelParams", self.no_fuel_params.state()),
            ("noFuelParamsLegacy", self.no_fuel_params_legacy.state()),
            ("damageHandling", self.damage_handling.state()),
            ("partDamageParams", self.part_damage_params.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored ifcs damage handling fields and their source evidence.
#[non_exhaustive]
pub struct IfcsDamageHandling<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    /// Original thrust threshold ang neg field.
    pub thrust_threshold_ang_neg: SourcedValue<InventoryVector3<'source>>,
    /// Original thrust threshold ang pos field.
    pub thrust_threshold_ang_pos: SourcedValue<InventoryVector3<'source>>,
    /// Original thrust threshold lin neg field.
    pub thrust_threshold_lin_neg: SourcedValue<InventoryVector3<'source>>,
    /// Original thrust threshold lin pos field.
    pub thrust_threshold_lin_pos: SourcedValue<InventoryVector3<'source>>,
}

impl<'source> IfcsDamageHandling<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let thrust_threshold_ang_neg =
            fields.declared_class("thrustThresholdAngNeg", notes, InventoryVector3::read)?;
        let thrust_threshold_ang_pos =
            fields.declared_class("thrustThresholdAngPos", notes, InventoryVector3::read)?;
        let thrust_threshold_lin_neg =
            fields.declared_class("thrustThresholdLinNeg", notes, InventoryVector3::read)?;
        let thrust_threshold_lin_pos =
            fields.declared_class("thrustThresholdLinPos", notes, InventoryVector3::read)?;
        let source = UnitSource::read(
            fields,
            [
                ("thrustThresholdAngNeg", thrust_threshold_ang_neg.state()),
                ("thrustThresholdAngPos", thrust_threshold_ang_pos.state()),
                ("thrustThresholdLinNeg", thrust_threshold_lin_neg.state()),
                ("thrustThresholdLinPos", thrust_threshold_lin_pos.state()),
            ],
        )?;
        Ok(Self {
            source,
            thrust_threshold_ang_neg,
            thrust_threshold_ang_pos,
            thrust_threshold_lin_neg,
            thrust_threshold_lin_pos,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored ifcs part damage fields and their source evidence.
#[non_exhaustive]
pub struct IfcsPartDamage<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    /// Original drag decrease coefficient field.
    pub drag_decrease_coefficient: SourcedValue<ResourceFloat32>,
    /// Original drag influence mapping field.
    pub drag_influence_mapping: SourcedValue<OrderedCurve<'source>>,
    /// Original lift decrease coefficient field.
    pub lift_decrease_coefficient: SourcedValue<ResourceFloat32>,
    /// Original lift influence mapping field.
    pub lift_influence_mapping: SourcedValue<OrderedCurve<'source>>,
    /// Original roll torque coefficient field.
    pub roll_torque_coefficient: SourcedValue<ResourceFloat32>,
    /// Original yaw torque coefficient field.
    pub yaw_torque_coefficient: SourcedValue<ResourceFloat32>,
}

impl<'source> IfcsPartDamage<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let drag_decrease_coefficient = fields.single("dragDecreaseCoefficient", notes)?;
        let drag_influence_mapping =
            fields.declared_class("dragInfluenceMapping", notes, OrderedCurve::read)?;
        let lift_decrease_coefficient = fields.single("liftDecreaseCoefficient", notes)?;
        let lift_influence_mapping =
            fields.declared_class("liftInfluenceMapping", notes, OrderedCurve::read)?;
        let roll_torque_coefficient = fields.single("rollTorqueCoefficient", notes)?;
        let yaw_torque_coefficient = fields.single("yawTorqueCoefficient", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("dragDecreaseCoefficient", drag_decrease_coefficient.state()),
                ("dragInfluenceMapping", drag_influence_mapping.state()),
                ("liftDecreaseCoefficient", lift_decrease_coefficient.state()),
                ("liftInfluenceMapping", lift_influence_mapping.state()),
                ("rollTorqueCoefficient", roll_torque_coefficient.state()),
                ("yawTorqueCoefficient", yaw_torque_coefficient.state()),
            ],
        )?;
        Ok(Self {
            source,
            drag_decrease_coefficient,
            drag_influence_mapping,
            lift_decrease_coefficient,
            lift_influence_mapping,
            roll_torque_coefficient,
            yaw_torque_coefficient,
        })
    }
}
