// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_resource_flow_unit::ResourceFlowUnit;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::degradation_curve::DegradationCurve;
use crate::catalog_v3::inventory_dimensions::InventoryVector3;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[path = "thruster_gimbal.rs"]
mod gimbal;
pub use gimbal::{Gimbal, GimbalAxis, GimbalOverride};

pub(in crate::catalog_v3) const COMPONENT: &str = "SCItemThrusterParams";

/// The authored operating parameters of one physical thruster component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct Thruster<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    /// Original operating field.
    pub operating: Operating<'source>,
    #[serde(flatten)]
    /// Original backwash field.
    pub backwash: Backwash<'source>,
    /// Original gimbal field.
    pub gimbal: SourcedValue<Gimbal<'source>>,
    /// Original gimbal interaction state overrides field.
    pub gimbal_interaction_state_overrides:
        SourcedValue<Vec<SourcedValue<GimbalOverride<'source>>>>,
    /// Original ship integrity loss modifier curve field.
    pub ship_integrity_loss_modifier_curve: SourcedValue<IntegrityCurve<'source>>,
    /// Original signature emitter params field.
    pub signature_emitter_params: SourcedValue<SignatureEmitter<'source>>,
}

impl<'source> Thruster<'source> {
    /// Borrows the original component type and remaining fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the original physical Record and component position.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    pub(in crate::catalog_v3) fn read(
        owner: ComponentOwner,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let operating = Operating::read(fields, notes)?;
        let backwash = Backwash::read(fields, notes)?;
        let gimbal = fields.declared_class("gimbal", notes, Gimbal::read)?;
        let gimbal_interaction_state_overrides = fields.declared_class_array(
            "gimbalInteractionStateOverrides",
            notes,
            GimbalOverride::read,
        )?;
        let ship_integrity_loss_modifier_curve = fields.declared_class(
            "shipIntegrityLossModifierCurve",
            notes,
            IntegrityCurve::read,
        )?;
        let signature_emitter_params =
            fields.declared_pointer("signatureEmitterParams", notes, SignatureEmitter::read)?;
        let source = UnitSource::read(
            fields,
            operating
                .projected()
                .into_iter()
                .chain(backwash.projected())
                .chain([
                    ("gimbal", gimbal.state()),
                    (
                        "gimbalInteractionStateOverrides",
                        gimbal_interaction_state_overrides.state(),
                    ),
                    (
                        "shipIntegrityLossModifierCurve",
                        ship_integrity_loss_modifier_curve.state(),
                    ),
                    ("signatureEmitterParams", signature_emitter_params.state()),
                ]),
        )?;
        Ok(Self {
            owner,
            source,
            operating,
            backwash,
            gimbal,
            gimbal_interaction_state_overrides,
            ship_integrity_loss_modifier_curve,
            signature_emitter_params,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored operating fields and their source evidence.
#[non_exhaustive]
pub struct Operating<'source> {
    /// Original thrust capacity field.
    pub thrust_capacity: SourcedValue<ResourceFloat32>,
    /// Original thrust capacity new field.
    pub thrust_capacity_new: SourcedValue<ResourceFloat32>,
    /// Original thruster type field.
    pub thruster_type: SourcedValue<&'source str>,
    #[serde(rename = "onlyActiveInVTOL")]
    /// Original only active in vtol field.
    pub only_active_in_vtol: SourcedValue<bool>,
    /// Original min health thrust multiplier field.
    pub min_health_thrust_multiplier: SourcedValue<ResourceFloat32>,
    /// Original no fuel threshold ratio field.
    pub no_fuel_threshold_ratio: SourcedValue<ResourceFloat32>,
    /// Original thruster strength smoothing field.
    pub thruster_strength_smoothing: SourcedValue<ResourceFloat32>,
    #[serde(rename = "fuelBurnRatePer10KNewton")]
    /// Original fuel burn rate per 10k newton field.
    pub fuel_burn_rate_per_10k_newton: SourcedValue<ResourceFloat32>,
    #[serde(rename = "fuelBurnRatePer10KNewtonRN")]
    /// Original fuel burn rate per 10k newton rn field.
    pub fuel_burn_rate_per_10k_newton_rn: SourcedValue<ResourceFlowUnit<'source>>,
    /// Original thruster anim driver field.
    pub thruster_anim_driver: SourcedValue<&'source str>,
    /// Original nozzle animation field.
    pub nozzle_animation: SourcedValue<&'source str>,
}

impl<'source> Operating<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            thrust_capacity: fields.single("thrustCapacity", notes)?,
            thrust_capacity_new: fields.single("thrustCapacityNew", notes)?,
            thruster_type: fields.borrowed_declared_enum("thrusterType", notes)?,
            only_active_in_vtol: fields.boolean("onlyActiveInVTOL", notes)?,
            min_health_thrust_multiplier: fields.single("minHealthThrustMultiplier", notes)?,
            no_fuel_threshold_ratio: fields.single("noFuelThresholdRatio", notes)?,
            thruster_strength_smoothing: fields.single("thrusterStrengthSmoothing", notes)?,
            fuel_burn_rate_per_10k_newton: fields.single("fuelBurnRatePer10KNewton", notes)?,
            fuel_burn_rate_per_10k_newton_rn: fields.declared_pointer(
                "fuelBurnRatePer10KNewtonRN",
                notes,
                ResourceFlowUnit::read,
            )?,
            thruster_anim_driver: fields.borrowed_declared_enum("thrusterAnimDriver", notes)?,
            nozzle_animation: fields.borrowed_string("nozzleAnimation", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 11] {
        [
            ("thrustCapacity", self.thrust_capacity.state()),
            ("thrustCapacityNew", self.thrust_capacity_new.state()),
            ("thrusterType", self.thruster_type.state()),
            ("onlyActiveInVTOL", self.only_active_in_vtol.state()),
            (
                "minHealthThrustMultiplier",
                self.min_health_thrust_multiplier.state(),
            ),
            ("noFuelThresholdRatio", self.no_fuel_threshold_ratio.state()),
            (
                "thrusterStrengthSmoothing",
                self.thruster_strength_smoothing.state(),
            ),
            (
                "fuelBurnRatePer10KNewton",
                self.fuel_burn_rate_per_10k_newton.state(),
            ),
            (
                "fuelBurnRatePer10KNewtonRN",
                self.fuel_burn_rate_per_10k_newton_rn.state(),
            ),
            ("thrusterAnimDriver", self.thruster_anim_driver.state()),
            ("nozzleAnimation", self.nozzle_animation.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored backwash fields and their source evidence.
#[non_exhaustive]
pub struct Backwash<'source> {
    /// Original automate backwash size field.
    pub automate_backwash_size: SourcedValue<bool>,
    #[serde(rename = "toggleThrusterBackwash")]
    /// Original enabled field.
    pub enabled: SourcedValue<bool>,
    /// Original thruster backwash size field.
    pub thruster_backwash_size: SourcedValue<InventoryVector3<'source>>,
    /// Original thruster backwash afterburner multiplier field.
    pub thruster_backwash_afterburner_multiplier: SourcedValue<ResourceFloat32>,
    /// Original thruster backwash max density field.
    pub thruster_backwash_max_density: SourcedValue<ResourceFloat32>,
    /// Original thruster backwash max resistance field.
    pub thruster_backwash_max_resistance: SourcedValue<ResourceFloat32>,
    /// Original thruster backwash max speed field.
    pub thruster_backwash_max_speed: SourcedValue<ResourceFloat32>,
}

impl<'source> Backwash<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            automate_backwash_size: fields.boolean("automateBackwashSize", notes)?,
            enabled: fields.boolean("toggleThrusterBackwash", notes)?,
            thruster_backwash_size: InventoryVector3::class_field(
                fields,
                "thrusterBackwashSize",
                notes,
            )?,
            thruster_backwash_afterburner_multiplier: fields
                .single("thrusterBackwashAfterburnerMultiplier", notes)?,
            thruster_backwash_max_density: fields.single("thrusterBackwashMaxDensity", notes)?,
            thruster_backwash_max_resistance: fields
                .single("thrusterBackwashMaxResistance", notes)?,
            thruster_backwash_max_speed: fields.single("thrusterBackwashMaxSpeed", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 7] {
        [
            ("automateBackwashSize", self.automate_backwash_size.state()),
            ("toggleThrusterBackwash", self.enabled.state()),
            ("thrusterBackwashSize", self.thruster_backwash_size.state()),
            (
                "thrusterBackwashAfterburnerMultiplier",
                self.thruster_backwash_afterburner_multiplier.state(),
            ),
            (
                "thrusterBackwashMaxDensity",
                self.thruster_backwash_max_density.state(),
            ),
            (
                "thrusterBackwashMaxResistance",
                self.thruster_backwash_max_resistance.state(),
            ),
            (
                "thrusterBackwashMaxSpeed",
                self.thruster_backwash_max_speed.state(),
            ),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
/// Original authored integrity curve fields and their source evidence.
#[non_exhaustive]
pub struct IntegrityCurve<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    #[serde(flatten)]
    /// Original curve field.
    pub curve: DegradationCurve,
}

impl<'source> IntegrityCurve<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let curve = DegradationCurve::read(fields, notes)?;
        let source = UnitSource::read(fields, curve.projected())?;
        Ok(Self { source, curve })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original authored signature emitter fields and their source evidence.
#[non_exhaustive]
pub struct SignatureEmitter<'source> {
    #[serde(flatten)]
    /// Original type and remaining authored fields.
    pub source: UnitSource<'source>,
    /// Original active signature field.
    pub active_signature: SourcedValue<ResourceFloat32>,
    /// Original full decay time field.
    pub full_decay_time: SourcedValue<ResourceFloat32>,
}

impl<'source> SignatureEmitter<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let active_signature = fields.single("activeSignature", notes)?;
        let full_decay_time = fields.single("fullDecayTime", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("activeSignature", active_signature.state()),
                ("fullDecayTime", full_decay_time.state()),
            ],
        )?;
        Ok(Self {
            source,
            active_signature,
            full_decay_time,
        })
    }
}

#[cfg(test)]
#[path = "thruster_tests.rs"]
mod tests;
