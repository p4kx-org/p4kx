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
use gimbal::{Gimbal, GimbalOverride};

pub(in crate::catalog_v3) const COMPONENT: &str = "SCItemThrusterParams";

type Single = SourcedValue<ResourceFloat32>;
type Label<'source> = SourcedValue<&'source str>;
type GimbalOverrides<'source> = SourcedValue<Vec<SourcedValue<GimbalOverride<'source>>>>;

/// The authored operating parameters of one physical thruster component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(in crate::catalog_v3) struct Thruster<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    operating: Operating<'source>,
    #[serde(flatten)]
    backwash: Backwash<'source>,
    gimbal: SourcedValue<Gimbal<'source>>,
    gimbal_interaction_state_overrides: GimbalOverrides<'source>,
    ship_integrity_loss_modifier_curve: SourcedValue<IntegrityCurve<'source>>,
    signature_emitter_params: SourcedValue<SignatureEmitter<'source>>,
}

impl<'source> Thruster<'source> {
    pub(in crate::catalog_v3) const fn owner(&self) -> &ComponentOwner {
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
struct Operating<'source> {
    thrust_capacity: Single,
    thrust_capacity_new: Single,
    thruster_type: Label<'source>,
    #[serde(rename = "onlyActiveInVTOL")]
    only_active_in_vtol: SourcedValue<bool>,
    min_health_thrust_multiplier: Single,
    no_fuel_threshold_ratio: Single,
    thruster_strength_smoothing: Single,
    #[serde(rename = "fuelBurnRatePer10KNewton")]
    fuel_burn_rate_per_10k_newton: Single,
    #[serde(rename = "fuelBurnRatePer10KNewtonRN")]
    fuel_burn_rate_per_10k_newton_rn: SourcedValue<ResourceFlowUnit<'source>>,
    thruster_anim_driver: Label<'source>,
    nozzle_animation: Label<'source>,
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
struct Backwash<'source> {
    automate_backwash_size: SourcedValue<bool>,
    #[serde(rename = "toggleThrusterBackwash")]
    enabled: SourcedValue<bool>,
    thruster_backwash_size: SourcedValue<InventoryVector3<'source>>,
    thruster_backwash_afterburner_multiplier: Single,
    thruster_backwash_max_density: Single,
    thruster_backwash_max_resistance: Single,
    thruster_backwash_max_speed: Single,
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
struct IntegrityCurve<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    curve: DegradationCurve,
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
struct SignatureEmitter<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    active_signature: Single,
    full_decay_time: Single,
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
