// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;

use p4kx_extraction::{DataForge, Record, RecordTree, TreeBuilder};

use crate::catalog_v3::catalog_durability;
use crate::contract_stats::{AmmoStats, ItemStats, ItemStatsFields};
use crate::record_fields::{component, structure, text};

mod gear;
mod mining;
mod parts;
mod resources;
mod walk;
mod weapon;

pub(crate) use weapon::{
    AMMO_CONTAINER, MagazineRead, MagazineRoute, PRIMARY_MAGAZINE_FIELD, WEAPON_COMPONENT, ammo,
};

const POWER_PLANT: &str = "PowerPlant";
const COOLER: &str = "Cooler";

/// Returns the stat blocks of the item `tree`, `None` when it has none of the blocks.
///
/// A block follows from the components of the item: the weapon, shield, quantum drive, missile,
/// suit armour, clothing, radar, mining laser and attachable modifier components, the tractor beam
/// action of the weapon, and the type `PowerPlant` or `Cooler` of its `AttachDef`. The ammunition
/// of a weapon is read from the Records the item links to, with `trees`; one that cannot be read
/// leaves the ammunition `null` and never fails the item.
pub(crate) fn read<'f>(
    tree: &RecordTree<'f>,
    forge: &'f DataForge<'f>,
    trees: &mut TreeBuilder<'f>,
) -> Option<ItemStats<'f>> {
    read_with_sources(
        tree,
        forge,
        trees,
        || catalog_durability::legacy_health(tree),
        |record, trees| Ok::<_, Infallible>(trees.build(record).ok().map(|tree| ammo(&tree))),
        |_| Ok(()),
    )
    .unwrap_or_else(|error| match error {})
}

pub(crate) fn read_with_sources<'f, E>(
    tree: &RecordTree<'f>,
    forge: &'f DataForge<'f>,
    trees: &mut TreeBuilder<'f>,
    health: impl FnOnce() -> Option<f32>,
    ammunition: impl FnOnce(&Record<'f>, &mut TreeBuilder<'f>) -> Result<Option<AmmoStats>, E>,
    observe_magazine: impl FnOnce(MagazineRead<'_, 'f>) -> Result<(), E>,
) -> Result<Option<ItemStats<'f>>, E> {
    let attach = structure(component(tree, "SAttachableComponentParams"), "AttachDef");
    let kind = text(attach, "Type");
    let is = |name: &str| kind.is_some_and(|kind| kind.eq_ignore_ascii_case(name));
    let weapon = weapon::weapon(tree, forge, trees, ammunition, observe_magazine)?;
    let shield = parts::shield(tree);
    let quantum_drive = parts::quantum_drive(tree);
    let resource_states = resources::states(tree);
    let power_plant = is(POWER_PLANT).then(|| resources::power_plant(&resource_states));
    let cooler = is(COOLER).then(|| resources::cooler(&resource_states));
    let missile = parts::missile(tree);
    let armor = gear::armor(tree, forge, trees);
    let clothing = gear::clothing(tree);
    let radar = gear::radar(tree);
    let tractor_beam = gear::tractor_beam(tree);
    let weapon_modifier = gear::weapon_modifier(tree);
    let mining_laser = mining::laser(tree);
    let mining_modifier = mining::modifier_block(tree);
    let mut stats = ItemStats {
        weapon,
        shield,
        quantum_drive,
        fields: ItemStatsFields {
            power_plant,
            cooler,
            missile,
            armor,
            clothing,
            radar,
            tractor_beam,
            weapon_modifier,
            mining_laser,
            mining_modifier,
            health: None,
            resource_states,
        },
    };
    if !stats.has_specialist() {
        return Ok(None);
    }
    stats.fields.health = health();
    Ok(Some(stats))
}
