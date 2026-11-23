// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForge, DataForgeError, Record, RecordTree, StructValue, TreeBuilder, Value,
};

use super::LinkedStatTarget;
use super::walk::owner_of;
use crate::contract_stats::{AmmoStats, WeaponStats};
use crate::record_fields::{component_occurrence, integer, number, structure};

#[path = "weapon_legacy_actions.rs"]
mod actions;
#[cfg(test)]
#[path = "weapon_deep_tests.rs"]
mod deep_tests;

use super::parts::damage;
use actions::read as fire_actions;
const AMMO_PARAMS: &str = "AmmoParams";
pub(super) const ENTITY: &str = "EntityClassDefinition";
pub(super) const AMMO_PARAMS_FIELD: &str = "ammoParamsRecord";
pub(super) const AMMO_CAPACITY_FIELD: &str = "maxAmmoCount";

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

pub(crate) const AMMO_CONTAINER: &str = "SAmmoContainerComponentParams";
pub(crate) const WEAPON_COMPONENT: &str = "SCItemWeaponComponentParams";
pub(crate) const PRIMARY_MAGAZINE_FIELD: &str = "ammoContainerRecord";

/// The original occurrences and read result used by the legacy primary selection.
#[derive(Clone, Copy)]
pub(crate) struct MagazineRead<'read, 'source> {
    weapon: &'read Value<'source>,
    route: MagazineRoute<'read, 'source>,
}

impl<'read, 'source> MagazineRead<'read, 'source> {
    pub(crate) const fn into_parts(self) -> (&'read Value<'source>, MagazineRoute<'read, 'source>) {
        (self.weapon, self.route)
    }

    const fn container(self) -> Option<&'read StructValue<'source>> {
        let value = match self.route {
            MagazineRoute::Own(value) => Some(value),
            MagazineRoute::Linked { container, .. } => container,
        };
        match value {
            Some(Value::Struct(fields)) => Some(fields),
            _ => None,
        }
    }
}

/// The actual on-weapon container or the one attempted target body.
#[derive(Clone, Copy)]
pub(crate) enum MagazineRoute<'read, 'source> {
    Own(&'read Value<'source>),
    Linked {
        caller: &'read Value<'source>,
        record: Record<'source>,
        tree: Result<&'read RecordTree<'source>, &'read DataForgeError>,
        container: Option<&'read Value<'source>>,
    },
}

pub(super) fn linked_target<'read, 'source>(
    selector: &'read Value<'source>,
    type_name: &str,
    forge: &'source DataForge<'source>,
) -> Option<LinkedStatTarget<'read, 'source>> {
    let Value::Reference(reference) = selector else {
        return None;
    };
    let target = reference.original_target();
    if target.record_type() != Some(type_name) {
        return None;
    }
    forge
        .record(target.record_index())
        .filter(|record| target.shares_source_with(record))
        .map(|record| LinkedStatTarget { selector, record })
}

/// Returns the capacity and ammunition of the container of `tree`.
fn loaded<'f, E>(
    container: Option<&StructValue<'f>>,
    forge: &'f DataForge<'f>,
    trees: &mut TreeBuilder<'f>,
    ammunition: impl FnOnce(
        LinkedStatTarget<'_, 'f>,
        &mut TreeBuilder<'f>,
    ) -> Result<Option<AmmoStats>, E>,
) -> Result<(Option<i64>, Option<AmmoStats>), E> {
    let target = container
        .and_then(|fields| fields.field(AMMO_PARAMS_FIELD))
        .and_then(|selector| linked_target(selector, AMMO_PARAMS, forge));
    let ammo = match target {
        Some(target) => ammunition(target, trees)?,
        None => None,
    };
    Ok((integer(container, AMMO_CAPACITY_FIELD), ammo))
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
    ammunition: impl FnOnce(
        LinkedStatTarget<'_, 'f>,
        &mut TreeBuilder<'f>,
    ) -> Result<Option<AmmoStats>, E>,
    observe_magazine: impl FnOnce(MagazineRead<'_, 'f>) -> Result<(), E>,
) -> Result<Option<WeaponStats<'f>>, E> {
    with_primary_magazine(tree, forge, trees, |weapon, magazine, trees| {
        let fire_actions = fire_actions(weapon);
        if let Some(magazine) = magazine {
            observe_magazine(magazine)?;
        }
        let (ammo_capacity, ammo) = loaded(
            magazine.and_then(MagazineRead::container),
            forge,
            trees,
            ammunition,
        )?;
        Ok(WeaponStats {
            fire_actions,
            ammo_capacity,
            ammo,
        })
    })
}

/// Observes the first weapon's original on-item or one-hop primary magazine route.
pub(crate) fn with_primary_magazine<'source, T, E>(
    tree: &RecordTree<'source>,
    forge: &'source DataForge<'source>,
    trees: &mut TreeBuilder<'source>,
    inspect: impl FnOnce(
        &StructValue<'source>,
        Option<MagazineRead<'_, 'source>>,
        &mut TreeBuilder<'source>,
    ) -> Result<T, E>,
) -> Result<Option<T>, E> {
    let Some((occurrence, weapon)) = component_occurrence(tree, WEAPON_COMPONENT) else {
        return Ok(None);
    };
    if let Some((original, _)) = component_occurrence(tree, AMMO_CONTAINER) {
        return inspect(
            weapon,
            Some(MagazineRead {
                weapon: occurrence,
                route: MagazineRoute::Own(original),
            }),
            trees,
        )
        .map(Some);
    }
    let linked = weapon
        .field(PRIMARY_MAGAZINE_FIELD)
        .and_then(|selector| linked_target(selector, ENTITY, forge));
    let Some(target) = linked else {
        return inspect(weapon, None, trees).map(Some);
    };
    let read = trees.build(&target.record);
    let container = read
        .as_ref()
        .ok()
        .and_then(|tree| component_occurrence(tree, AMMO_CONTAINER));
    inspect(
        weapon,
        Some(MagazineRead {
            weapon: occurrence,
            route: MagazineRoute::Linked {
                caller: target.selector,
                record: target.record,
                tree: read.as_ref(),
                container: container.map(|(value, _)| value),
            },
        }),
        trees,
    )
    .map(Some)
}
