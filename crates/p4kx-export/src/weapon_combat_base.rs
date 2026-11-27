// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::ammo_definition::{self, AmmoDefinition, PreparedAmmoDefinition};
use crate::catalog_v3::authored_ammo_inputs::AuthoredAmmoInputs;
use crate::catalog_v3::authored_damage::DamageInfo;
use crate::catalog_v3::catalog_item_components::ItemComponentTables;
use crate::catalog_v3::catalog_weapon_actions::{
    ProjectileCombatInputs, WeaponAction, WeaponActions, WeaponModeKind,
};
use crate::catalog_v3::catalog_weapon_projectile_ammo::WeaponProjectileAmmoSelection;
use crate::item_stats::WEAPON_COMPONENT;
use crate::{CatalogError, ExportAccountingError};
use p4kx_extraction::{CancelToken, DataForge, FieldOrigin, SourceUses, SourceValue, SourcedValue};

#[path = "weapon_combat_contract.rs"]
mod contract;
#[path = "weapon_combat_math.rs"]
mod math;

pub use crate::catalog_v3::authored_damage::DamageChannels;
pub use contract::{
    AlphaUnit, CombatModel, CombatUnits, DistanceUnit, DpsUnit, FireRateUnit, NominalBullet,
    NominalBulletDamage, WeaponCombatBase,
};

const FORMULA_VERSION: u8 = 1;

#[path = "countermeasure_emitter.rs"]
pub(crate) mod countermeasure_emitter;

use countermeasure_emitter::CountermeasureEmitter;

pub(crate) struct CatalogWeaponTables<'source> {
    pub(crate) ammunition: CatalogTable<AmmoDefinition<'source>>,
    pub(crate) combat: CatalogTable<WeaponCombatBase<'source>>,
    pub(crate) emitters: CatalogTable<CountermeasureEmitter>,
}

impl<'source> CatalogWeaponTables<'source> {
    pub(crate) fn build(
        forge: &DataForge<'_>,
        input: &OriginalCatalogInput<'_, 'source>,
        components: &ItemComponentTables<'source>,
        ammunition: Vec<PreparedAmmoDefinition<'source>>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let combat = rows(
            &input.origins().uses,
            &components.weapon_actions.rows,
            &components.projectile_ammo_selections.rows,
            &ammunition,
            cancel,
        )?;
        let emitters = countermeasure_emitter::table(
            forge,
            &input.origins().uses,
            &components.projectile_ammo_selections.rows,
            &ammunition,
            cancel,
        )?;
        Ok(Self {
            emitters,
            ammunition: ammo_definition::table(forge, ammunition),
            combat: CatalogTable::new(forge, &[WEAPON_COMPONENT], combat),
        })
    }
}

pub(crate) fn rows<'source>(
    sources: &SourceUses,
    actions: &[WeaponActions<'source>],
    selections: &[WeaponProjectileAmmoSelection],
    ammunition: &[PreparedAmmoDefinition<'source>],
    cancel: Option<&CancelToken>,
) -> Result<Vec<WeaponCombatBase<'source>>, CatalogError> {
    let selections: BTreeMap<_, _> = selections
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
    for weapon in actions {
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
    Ok(rows)
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
            units: CombatUnits::NOMINAL,
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
