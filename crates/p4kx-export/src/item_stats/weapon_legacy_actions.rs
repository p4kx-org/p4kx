// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice::Iter;

use p4kx_extraction::{StructValue, Value};

use super::super::parts::damage;
use crate::contract_stats::{FireAction, Spread};
use crate::record_fields::{elements, integer, number, structure, text};

const SEQUENCE_ENTRIES: &str = "sequenceEntries";
const PARALLEL_ACTIONS: &str = "weaponActions";

enum ChildLayout {
    Sequence,
    Direct,
}

struct ChildSlots<'tree, 'source> {
    slots: Iter<'tree, Value<'source>>,
    layout: ChildLayout,
}

impl<'tree, 'source> ChildSlots<'tree, 'source> {
    fn new(slots: &'tree [Value<'source>], layout: ChildLayout) -> Self {
        Self {
            slots: slots.iter(),
            layout,
        }
    }

    fn next(&mut self) -> Option<&'tree StructValue<'source>> {
        self.slots.find_map(|slot| {
            let Value::Struct(entry) = slot else {
                return None;
            };
            match self.layout {
                ChildLayout::Sequence => structure(Some(entry), "weaponAction"),
                ChildLayout::Direct => Some(entry),
            }
        })
    }
}

/// Flattens original legacy wrapper slots in their authored depth-first order.
pub(super) fn read<'source>(weapon: &StructValue<'source>) -> Vec<FireAction<'source>> {
    let mut pending = vec![ChildSlots::new(
        elements(Some(weapon), "fireActions"),
        ChildLayout::Direct,
    )];
    let mut fire_actions = Vec::new();
    while let Some(slots) = pending.last_mut() {
        let Some(action) = slots.next() else {
            pending.pop();
            continue;
        };
        if let Some(nested) = children(action) {
            pending.push(nested);
        } else {
            fire_actions.push(leaf(action));
        }
    }
    fire_actions
}

fn children<'tree, 'source>(
    action: &'tree StructValue<'source>,
) -> Option<ChildSlots<'tree, 'source>> {
    if action.field(SEQUENCE_ENTRIES).is_some() {
        Some(ChildSlots::new(
            elements(Some(action), SEQUENCE_ENTRIES),
            ChildLayout::Sequence,
        ))
    } else if action.field(PARALLEL_ACTIONS).is_some() {
        Some(ChildSlots::new(
            elements(Some(action), PARALLEL_ACTIONS),
            ChildLayout::Direct,
        ))
    } else {
        None
    }
}

fn leaf<'source>(action: &StructValue<'source>) -> FireAction<'source> {
    let inner = structure(Some(action), "weaponAction");
    let launch =
        structure(Some(action), "launchParams").or_else(|| structure(inner, "launchParams"));
    let value = |name| number(Some(action), name).or_else(|| number(inner, name));
    FireAction {
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
    }
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
