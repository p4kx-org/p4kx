// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{
    CancelToken, DataForge, DecimalU64, FieldOrigin, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::ammo_definition::{self, AmmoDefinition, PreparedAmmoDefinition};
use crate::catalog_v3::authored_ammo_inputs::AuthoredAmmoInputs;
use crate::catalog_v3::authored_damage::{DamageChannels, DamageInfo};
use crate::catalog_v3::catalog_item_components::ItemComponentTables;
use crate::catalog_v3::catalog_weapon_actions::{
    ProjectileCombatInputs, WeaponAction, WeaponActions, WeaponModeKind,
};
use crate::catalog_v3::catalog_weapon_projectile_ammo::WeaponProjectileAmmoSelection;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::item_stats::WEAPON_COMPONENT;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

#[path = "weapon_combat_math.rs"]
mod math;

const FORMULA_VERSION: u8 = 1;

/// Nominal combat base values keyed to a physical weapon component and fire mode.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct WeaponCombatBase<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[schemars(range(min = 1, max = 1))]
    formula_version: u8,
    units: CombatUnits,
    mode_index: SourcedValue<u32>,
    action_source_value_ordinal: Option<SourcedValue<DecimalU64>>,
    action_source_type: Option<SourcedValue<&'source str>>,
    ammunition_record_index: Option<SourcedValue<u32>>,
    model: SourcedValue<CombatModel>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct CombatUnits {
    fire_rate: FireRateUnit,
    alpha: AlphaUnit,
    nominal_dps: DpsUnit,
    nominal_flight_distance: DistanceUnit,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum FireRateUnit {
    DischargesPerMinute,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum AlphaUnit {
    DamagePoints,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum DpsUnit {
    DamagePointsPerSecond,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum DistanceUnit {
    Metres,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
enum CombatModel {
    NominalBullet { values: Box<NominalBullet> },
    AmmoSelectionUnavailable,
    AmmoDefinitionUnavailable,
    ActionUnavailable,
    UnsupportedAction,
    Burst,
    Beam,
    Charge,
    Conditional,
    ProjectileUnavailable,
    UnsupportedProjectile,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct NominalBullet {
    fire_rate_rpm: SourcedValue<i32>,
    projectiles_per_discharge: SourcedValue<i32>,
    launcher_damage_multiplier: SourcedValue<ResourceFloat32>,
    nominal_flight_distance: SourcedValue<f64>,
    damage: SourcedValue<NominalBulletDamage>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct NominalBulletDamage {
    alpha: DamageChannels<SourcedValue<f64>>,
    nominal_dps: DamageChannels<SourcedValue<f64>>,
}

pub(crate) struct CatalogWeaponTables<'source> {
    pub(crate) ammunition: CatalogTable<AmmoDefinition<'source>>,
    pub(crate) combat: CatalogTable<WeaponCombatBase<'source>>,
}

impl<'source> CatalogWeaponTables<'source> {
    pub(crate) fn build(
        forge: &DataForge<'_>,
        input: &OriginalCatalogInput<'_, 'source>,
        components: &ItemComponentTables<'source>,
        ammunition: Vec<PreparedAmmoDefinition<'source>>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let combat = table(
            forge,
            &input.origins().uses,
            components,
            &ammunition,
            cancel,
        )?;
        Ok(Self {
            ammunition: ammo_definition::table(forge, ammunition),
            combat,
        })
    }
}

fn table<'source>(
    forge: &DataForge<'_>,
    sources: &SourceUses,
    components: &ItemComponentTables<'source>,
    ammunition: &[PreparedAmmoDefinition<'source>],
    cancel: Option<&CancelToken>,
) -> Result<CatalogTable<WeaponCombatBase<'source>>, CatalogError> {
    let selections: BTreeMap<_, _> = components
        .projectile_ammo_selections
        .rows
        .iter()
        .map(|row| {
            (
                (
                    row.owner().key(),
                    row.action_source_value_ordinal()
                        .value()
                        .map(|value| value.get()),
                ),
                row,
            )
        })
        .collect();
    let ammunition: BTreeMap<_, _> = ammunition
        .iter()
        .map(|definition| (definition.physical_index, definition.inputs()))
        .collect();
    let inputs = CombatInputs {
        sources,
        selections,
        ammunition,
    };
    let mut rows = Vec::new();
    for weapon in &components.weapon_actions.rows {
        let Some(slots) = weapon.fire_actions().value() else {
            continue;
        };
        for (index, slot) in slots.iter().enumerate() {
            if cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let index = u32::try_from(index).map_err(|_| ExportAccountingError::CounterOverflow)?;
            rows.push(inputs.mode(weapon, index, slot)?);
        }
    }
    Ok(CatalogTable::new(forge, &[WEAPON_COMPONENT], rows))
}

type ModeKey = ((Option<u32>, Option<u32>), Option<u64>);

struct CombatInputs<'rows, 'source> {
    sources: &'rows SourceUses,
    selections: BTreeMap<ModeKey, &'rows WeaponProjectileAmmoSelection>,
    ammunition: BTreeMap<u32, &'rows AuthoredAmmoInputs<'source>>,
}

impl<'source> CombatInputs<'_, 'source> {
    fn mode(
        &self,
        weapon: &WeaponActions<'source>,
        index: u32,
        slot: &SourcedValue<u32>,
    ) -> Result<WeaponCombatBase<'source>, CatalogError> {
        let action = slot
            .value()
            .map(|node| {
                weapon
                    .mode(*node)
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)
            })
            .transpose()?;
        let ordinal = action.map(WeaponAction::source_ordinal);
        let selection = self
            .selections
            .get(&(
                weapon.owner().key(),
                ordinal
                    .and_then(SourcedValue::value)
                    .map(|value| value.get()),
            ))
            .copied();
        let ammo_index = selection.and_then(WeaponProjectileAmmoSelection::selected_ammo_index);
        let mode_origins = [
            weapon.fire_actions().origin(),
            slot.origin(),
            action.map_or_else(|| slot.origin(), |node| node.source_type().origin()),
        ];
        let model = match action.map(WeaponAction::mode_kind) {
            Some(WeaponModeKind::SimpleShot) => {
                match action.and_then(WeaponAction::combat_inputs) {
                    Some(inputs) => calculate(
                        self.sources,
                        &inputs,
                        selection,
                        ammo_index,
                        &self.ammunition,
                        mode_origins,
                    )?,
                    None => model(self.sources, CombatModel::UnsupportedAction, &mode_origins)?,
                }
            }
            other => model(self.sources, unavailable_mode(other), &mode_origins)?,
        };
        Ok(WeaponCombatBase {
            owner: weapon.owner().clone(),
            formula_version: FORMULA_VERSION,
            units: CombatUnits {
                fire_rate: FireRateUnit::DischargesPerMinute,
                alpha: AlphaUnit::DamagePoints,
                nominal_dps: DpsUnit::DamagePointsPerSecond,
                nominal_flight_distance: DistanceUnit::Metres,
            },
            mode_index: self
                .sources
                .value_with_origins(SourceValue::Present(index), &mode_origins[..2])?,
            action_source_value_ordinal: ordinal.cloned(),
            action_source_type: action.map(|node| node.source_type().clone()),
            ammunition_record_index: ammo_index.cloned(),
            model,
        })
    }
}

fn model(
    sources: &SourceUses,
    value: CombatModel,
    origins: &[&FieldOrigin],
) -> Result<SourcedValue<CombatModel>, CatalogError> {
    Ok(sources.value_with_origins(SourceValue::Present(value), origins)?)
}

const fn unavailable_mode(kind: Option<WeaponModeKind>) -> CombatModel {
    match kind {
        Some(WeaponModeKind::Burst) => CombatModel::Burst,
        Some(WeaponModeKind::Beam) => CombatModel::Beam,
        Some(WeaponModeKind::Charge) => CombatModel::Charge,
        Some(WeaponModeKind::Conditional) => CombatModel::Conditional,
        Some(_) => CombatModel::UnsupportedAction,
        None => CombatModel::ActionUnavailable,
    }
}

fn calculate(
    sources: &SourceUses,
    action: &ProjectileCombatInputs<'_>,
    selection: Option<&WeaponProjectileAmmoSelection>,
    ammo_index: Option<&SourcedValue<u32>>,
    ammunition: &BTreeMap<u32, &AuthoredAmmoInputs<'_>>,
    mode_origins: [&FieldOrigin; 3],
) -> Result<SourcedValue<CombatModel>, CatalogError> {
    let [modes, slot, kind] = mode_origins;
    let Some(selection) = selection else {
        return model(
            sources,
            CombatModel::AmmoSelectionUnavailable,
            &mode_origins,
        );
    };
    let selection_origin = selection.origin();
    let Some(ammo_index) = ammo_index else {
        return model(
            sources,
            CombatModel::AmmoSelectionUnavailable,
            &[modes, slot, kind, selection_origin],
        );
    };
    let origins = [modes, slot, kind, selection_origin, ammo_index.origin()];
    let Some(ammo) = ammo_index.value().and_then(|index| ammunition.get(index)) else {
        return model(sources, CombatModel::AmmoDefinitionUnavailable, &origins);
    };
    nominal_bullet(sources, action, ammo, &origins)
}

fn nominal_bullet(
    sources: &SourceUses,
    action: &ProjectileCombatInputs<'_>,
    ammo: &AuthoredAmmoInputs<'_>,
    selection: &[&FieldOrigin; 5],
) -> Result<SourcedValue<CombatModel>, CatalogError> {
    let [modes, slot, kind, route, index] = *selection;
    let projectile = ammo.projectile();
    let origins = [modes, slot, kind, route, index, projectile.origin()];
    let Some(projectile_body) = projectile.value() else {
        return model(sources, CombatModel::ProjectileUnavailable, &origins);
    };
    let Some(damage) = projectile_body.bullet_damage() else {
        return model(sources, CombatModel::UnsupportedProjectile, &origins);
    };
    let nominal_flight_distance = math::flight_distance(sources, ammo, &origins)?;
    let damage_origins = [
        modes,
        slot,
        kind,
        route,
        index,
        projectile.origin(),
        damage.origin(),
    ];
    let output = match damage.value() {
        Some(body) => match DamageInfo::from_authored(body) {
            Some(info) => {
                SourceValue::Present(math::bullet_damage(sources, action, info, &damage_origins)?)
            }
            None => SourceValue::Unsupported,
        },
        None => math::unavailable(damage.state()),
    };
    model(
        sources,
        CombatModel::NominalBullet {
            values: Box::new(NominalBullet {
                fire_rate_rpm: action.fire_rate.clone(),
                projectiles_per_discharge: action.pellet_count.clone(),
                launcher_damage_multiplier: action.damage_multiplier.clone(),
                nominal_flight_distance,
                damage: sources.value_with_origins(output, &damage_origins)?,
            }),
        },
        &origins,
    )
}
