// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::event_actions::EventActions;
use super::initial_damage::InitialDamage;
use super::link::HealthLink;
use super::observed::Observed;
use crate::CatalogError;
use crate::catalog_v3::authored_explosion::{AuthoredExplosion, EXPLOSION_TYPES};
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

/// Selected Health component fields with original ordered lifecycle slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Lifecycle<'source> {
    initial_damage: SourcedValue<InitialDamage<'source>>,
    hit_manipulation_params: SourcedValue<HitManipulation>,
    health_level_states: SourcedValue<Vec<SourcedValue<HealthDamageState<'source>>>>,
    death_explosion_params: SourcedValue<AuthoredExplosion<'source>>,
    death_gas_composition: SourcedValue<Vec<SourcedValue<GasMass<'source>>>>,
    interaction_locks: SourcedValue<Vec<SourcedValue<InteractionLock<'source>>>>,
    #[serde(flatten)]
    event_actions: EventActions<'source>,
}

impl<'source> Lifecycle<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            initial_damage: source.owned(
                "InitialDamage",
                &["SInitialDamage"],
                notes,
                InitialDamage::read,
            )?,
            hit_manipulation_params: source.owned(
                "hitManipulationParams",
                &["HitManipulationParams"],
                notes,
                HitManipulation::read,
            )?,
            health_level_states: source.array(
                "HealthLevelStates",
                &["HealthDamageState"],
                notes,
                HealthDamageState::read,
            )?,
            death_explosion_params: source.owned(
                "DeathExplosionParams",
                EXPLOSION_TYPES,
                notes,
                AuthoredExplosion::read,
            )?,
            death_gas_composition: source.array(
                "DeathGasComposition",
                &["SGasMass"],
                notes,
                GasMass::read,
            )?,
            interaction_locks: source.array(
                "InteractionLocks",
                &["SHealthInteractionLockParams"],
                notes,
                InteractionLock::read,
            )?,
            event_actions: EventActions::read(source, notes)?,
        })
    }
}

/// Separate authored controls for AI and player hits.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HitManipulation {
    ai: SourcedValue<HitMultiplier>,
    player: SourcedValue<HitMultiplier>,
}

impl HitManipulation {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            ai: source.owned(
                "ai",
                &["HitManipulationParamsBase"],
                notes,
                HitMultiplier::read,
            )?,
            player: source.owned(
                "player",
                &["HitManipulationParamsBase"],
                notes,
                HitMultiplier::read,
            )?,
        })
    }
}

/// The original hit damage multiplier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HitMultiplier {
    damage_multiplier: SourcedValue<ResourceFloat32>,
}

impl HitMultiplier {
    pub(super) fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            damage_multiplier: source.single("damageMultiplier", notes)?,
        })
    }
}

/// One authored `HealthLevelStates` occurrence and its original links.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct HealthDamageState<'source> {
    geometry_tag: SourcedValue<String>,
    effect_trigger: SourcedValue<HealthLink<'source>>,
    effect_tag: SourcedValue<HealthLink<'source>>,
}

impl<'source> HealthDamageState<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            geometry_tag: source.string("geometryTag", notes)?,
            effect_trigger: source.link("effectTrigger", notes)?,
            effect_tag: source.link("effectTag", notes)?,
        })
    }
}

/// One authored death-gas slot, its stored mass and original gas link.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct GasMass<'source> {
    gas: SourcedValue<HealthLink<'source>>,
    mass: SourcedValue<ResourceFloat32>,
}

impl<'source> GasMass<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            gas: source.link("Gas", notes)?,
            mass: source.single("Mass", notes)?,
        })
    }
}

/// One authored interaction lock and the two stored Health ratios.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct InteractionLock<'source> {
    interaction: SourcedValue<HealthLink<'source>>,
    min_health_ratio: SourcedValue<ResourceFloat32>,
    max_health_ratio: SourcedValue<ResourceFloat32>,
}

impl<'source> InteractionLock<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            interaction: source.link("Interaction", notes)?,
            min_health_ratio: source.single("MinHealthRatio", notes)?,
            max_health_ratio: source.single("MaxHealthRatio", notes)?,
        })
    }
}
