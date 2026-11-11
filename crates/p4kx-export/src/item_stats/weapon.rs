// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Record, RecordTree, StructValue, TreeBuilder, Value};

use super::parts::damage;
use super::walk::owner_of;
use crate::contract_stats::{AmmoStats, FireAction, Spread, WeaponStats};
use crate::record_fields::{component, elements, integer, number, reference, structure, text};

const SEQUENCE_ENTRIES: &str = "sequenceEntries";
const PARALLEL_ACTIONS: &str = "weaponActions";
const AMMO_PARAMS: &str = "AmmoParams";
const ENTITY: &str = "EntityClassDefinition";

fn collect<'f>(action: &StructValue<'f>, actions: &mut Vec<FireAction<'f>>) {
    if action.field(SEQUENCE_ENTRIES).is_some() {
        for entry in elements(Some(action), SEQUENCE_ENTRIES) {
            if let Value::Struct(entry) = entry
                && let Some(inner) = structure(Some(entry), "weaponAction")
            {
                collect(inner, actions);
            }
        }
        return;
    }
    if action.field(PARALLEL_ACTIONS).is_some() {
        for entry in elements(Some(action), PARALLEL_ACTIONS) {
            if let Value::Struct(entry) = entry {
                collect(entry, actions);
            }
        }
        return;
    }
    let inner = structure(Some(action), "weaponAction");
    let launch =
        structure(Some(action), "launchParams").or_else(|| structure(inner, "launchParams"));
    let value = |name| number(Some(action), name).or_else(|| number(inner, name));
    actions.push(FireAction {
        kind: action.type_name(),
        name: text(Some(action), "name").or_else(|| text(inner, "name")),
        fire_rate: value("fireRate"),
        pellets: integer(launch, "pelletCount").or_else(|| integer(launch, "pellets")),
        ammo_cost: integer(launch, "ammoCost"),
        heat_per_shot: value("heatPerShot"),
        charge_time: value("chargeTime"),
        overcharge_time: value("overchargeTime"),
        cooldown_time: value("cooldownTime"),
        hit_type: text(Some(action), "hitType").or_else(|| text(inner, "hitType")),
        full_damage_range: value("fullDamageRange"),
        zero_damage_range: value("zeroDamageRange"),
        damage_per_second: damage(structure(Some(action), "damagePerSecond"))
            .or_else(|| damage(structure(inner, "damagePerSecond"))),
        spread: spread(launch).or_else(|| spread(Some(action))),
    });
}

fn spread(launch: Option<&StructValue<'_>>) -> Option<Spread> {
    let spread = structure(launch, "spreadParams")?;
    let value = |name| number(Some(spread), name);
    Some(Spread {
        min: value("min"),
        max: value("max"),
        first_attack: value("firstAttack"),
        attack: value("attack"),
        decay: value("decay"),
    })
}

pub(crate) fn ammo(tree: &RecordTree<'_>) -> AmmoStats {
    let projectile = structure(Some(tree.root()), "projectileParams");
    let explosion = owner_of(tree.root(), "explosionParams")
        .and_then(|owner| structure(Some(owner), "explosionParams"));
    AmmoStats {
        speed: number(Some(tree.root()), "speed"),
        lifetime: number(Some(tree.root()), "lifetime"),
        damage: damage(structure(projectile, "damage")),
        detonation_damage: damage(explosion),
        detonation_min_radius: number(explosion, "minRadius"),
        detonation_max_radius: number(explosion, "maxRadius"),
    }
}

const CONTAINER: &str = "SAmmoContainerComponentParams";

/// Returns the tree of the Record `reference` links to when it has the struct `type_name` and can
/// be built.
pub(super) fn linked<'f>(
    reference: Option<crate::guid_text::CatalogGuid>,
    type_name: &str,
    forge: &'f DataForge<'f>,
    trees: &mut TreeBuilder<'f>,
) -> Option<RecordTree<'f>> {
    let record = linked_record(reference, type_name, forge)?;
    trees.build(&record).ok()
}

fn linked_record<'f>(
    reference: Option<crate::guid_text::CatalogGuid>,
    type_name: &str,
    forge: &'f DataForge<'f>,
) -> Option<Record<'f>> {
    let record = forge.record_by_guid(reference?.0)?;
    (record.type_name().ok()? == type_name).then_some(record)
}

/// Returns the capacity and ammunition of the container of `tree`.
fn loaded<'f, E>(
    container: Option<&StructValue<'f>>,
    forge: &'f DataForge<'f>,
    trees: &mut TreeBuilder<'f>,
    ammunition: impl FnOnce(&Record<'f>, &mut TreeBuilder<'f>) -> Result<Option<AmmoStats>, E>,
) -> Result<(Option<i64>, Option<AmmoStats>), E> {
    let target = linked_record(reference(container, "ammoParamsRecord"), AMMO_PARAMS, forge);
    let ammo = match target {
        Some(record) => ammunition(&record, trees)?,
        None => None,
    };
    Ok((integer(container, "maxAmmoCount"), ammo))
}

/// Returns the weapon block of the item `tree`; `None` without a weapon component.
///
/// A ship weapon holds its ammo container itself; a personal weapon links the magazine entity
/// through `ammoContainerRecord`, followed one hop. A Record that is missing, has another struct
/// or cannot be built leaves the ammunition `None`. Errors from the ammunition reader propagate.
pub(super) fn weapon<'f, E>(
    tree: &RecordTree<'f>,
    forge: &'f DataForge<'f>,
    trees: &mut TreeBuilder<'f>,
    ammunition: impl FnOnce(&Record<'f>, &mut TreeBuilder<'f>) -> Result<Option<AmmoStats>, E>,
) -> Result<Option<WeaponStats<'f>>, E> {
    let Some(weapon) = component(tree, "SCItemWeaponComponentParams") else {
        return Ok(None);
    };
    let mut fire_actions = Vec::new();
    for action in elements(Some(weapon), "fireActions") {
        if let Value::Struct(action) = action {
            collect(action, &mut fire_actions);
        }
    }
    let (ammo_capacity, ammo) = if let Some(container) = component(tree, CONTAINER) {
        loaded(Some(container), forge, trees, ammunition)?
    } else {
        let magazine = linked(
            reference(Some(weapon), "ammoContainerRecord"),
            ENTITY,
            forge,
            trees,
        );
        loaded(
            magazine
                .as_ref()
                .and_then(|magazine| component(magazine, CONTAINER)),
            forge,
            trees,
            ammunition,
        )?
    };
    Ok(Some(WeaponStats {
        fire_actions,
        ammo_capacity,
        ammo,
    }))
}
