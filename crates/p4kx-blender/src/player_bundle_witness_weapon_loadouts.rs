// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io;

use p4kx_extraction::{Record, Value};
use serde_json::Value as Json;

use crate::player_bundle_wire::Row;

use super::{Forge, Weapon, attach_definition, sourced_index, xml};

#[derive(Default)]
pub(super) struct Coverage {
    records: BTreeMap<u64, Json>,
    links: BTreeMap<(u64, u64), Json>,
}

type PhysicalWeapon = (u32, usize);

impl Coverage {
    pub(super) fn record(&mut self, row: &Row<'_>) -> io::Result<()> {
        match row {
            Row::Loadout { index, value } => {
                if self
                    .records
                    .insert(*index as u64, serde_json::to_value(value)?)
                    .is_some()
                {
                    return Err(io::Error::other("duplicate Player loadout slot"));
                }
            }
            Row::WeaponMotionLoadout { value } => {
                let wire = serde_json::to_value(value)?;
                let location = (
                    sourced_index(&wire, "loadout")?,
                    sourced_index(&wire, "declaration")?,
                );
                if self.links.insert(location, wire).is_some() {
                    return Err(io::Error::other("duplicate weapon loadout posting"));
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn verify(
        &self,
        originals: &BTreeMap<u64, xml::Document>,
        forge: &Forge<'_>,
        weapons: &BTreeMap<usize, Weapon>,
    ) -> io::Result<()> {
        let mut classes = Classes::new(forge);
        let mut visited = BTreeSet::new();
        let mut physical = BTreeSet::new();
        for (&loadout, row) in &self.records {
            let Some(document) = row["value"]["document"]
                .as_u64()
                .filter(|_| row["value"]["type"] == "present")
            else {
                continue;
            };
            let rows = originals
                .get(&document)
                .and_then(xml::Document::loadout)
                .ok_or_else(|| io::Error::other("original admitted loadout not found"))?;
            for (declaration, item) in rows.iter().enumerate() {
                let Some(name) = item.as_deref().filter(|value| !value.is_empty()) else {
                    continue;
                };
                let (ambiguous, expected) = classes.weapons(name)?;
                if expected.is_empty() {
                    continue;
                }
                let location = (loadout, declaration as u64);
                let row = self.links.get(&location).ok_or_else(|| {
                    io::Error::other("missing weapon link for source loadout occurrence")
                })?;
                verify_link(row, ambiguous, expected, weapons)?;
                visited.insert(location);
                physical.extend(expected.iter().copied());
            }
        }
        let admitted: BTreeSet<_> = weapons
            .values()
            .map(|weapon| (weapon.record, weapon.component))
            .collect();
        if visited.len() != self.links.len() || admitted != physical {
            return Err(io::Error::other(
                "weapon source/Loadout reconciliation coverage differs",
            ));
        }
        Ok(())
    }
}

fn verify_link(
    row: &Json,
    ambiguous: bool,
    expected: &[PhysicalWeapon],
    weapons: &BTreeMap<usize, Weapon>,
) -> io::Result<()> {
    if row["ambiguousTarget"]["value"].as_bool() != Some(ambiguous) {
        return Err(io::Error::other("original weapon class ambiguity differs"));
    }
    let mut found = Vec::new();
    let values = row["weapons"]
        .as_array()
        .ok_or_else(|| io::Error::other("weapon candidate order is absent"))?;
    for wire in values {
        let index = wire["value"]
            .as_u64()
            .ok_or_else(|| io::Error::other("weapon candidate index unavailable"))?;
        let index = usize::try_from(index).map_err(io::Error::other)?;
        let original = weapons
            .get(&index)
            .ok_or_else(|| io::Error::other("weapon candidate has no original record/component"))?;
        found.push((original.record, original.component));
    }
    if found != expected {
        return Err(io::Error::other(
            "physical weapon candidates differ from the Loadout source",
        ));
    }
    Ok(())
}

struct Classes<'a> {
    records: HashMap<String, Vec<Record<'a>>>,
    weapons: HashMap<String, (bool, Vec<PhysicalWeapon>)>,
}

impl<'a> Classes<'a> {
    fn new(forge: &'a Forge<'a>) -> Self {
        let mut records = HashMap::<_, Vec<_>>::new();
        for record in forge.source.records() {
            if record.type_name().ok() != Some("EntityClassDefinition") {
                continue;
            }
            let Some(name) = record
                .name()
                .ok()
                .and_then(|name| name.strip_prefix("EntityClassDefinition."))
            else {
                continue;
            };
            records
                .entry(name.to_ascii_lowercase())
                .or_default()
                .push(record);
        }
        Self {
            records,
            weapons: HashMap::new(),
        }
    }

    fn weapons(&mut self, name: &str) -> io::Result<(bool, &[PhysicalWeapon])> {
        let folded = name.to_ascii_lowercase();
        let key = folded
            .strip_prefix("entityclassdefinition.")
            .unwrap_or(&folded);
        let result = match self.weapons.entry(key.to_owned()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let records = self.records.get(key).map_or(&[][..], Vec::as_slice);
                let mut components = Vec::new();
                for record in records {
                    let tree = record.tree().map_err(|_| {
                        io::Error::other("selected original Loadout class cannot be read")
                    })?;
                    if let Some(value @ Value::Array(slots)) = tree.field("Components") {
                        for slot in 0..slots.len() {
                            let Some(definition) = attach_definition(value, slot) else {
                                continue;
                            };
                            if matches!(definition.field("Type"), Some(Value::String(name) | Value::Enum(name)) if name.eq_ignore_ascii_case("WeaponPersonal"))
                            {
                                components.push((record.index(), slot));
                            }
                        }
                    }
                }
                entry.insert((records.len() > 1, components))
            }
        };
        Ok((result.0, &result.1))
    }
}
