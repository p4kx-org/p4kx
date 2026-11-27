// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::coverage::{self, SHIELD_GENERATORS};
use super::error::{self, Error, Kind};
use super::joins::Index;
use super::published_model::CatalogTarget;

pub(super) fn index<'a>(
    catalog: &'a Value,
    target: &CatalogTarget,
    owners: &Index<'_>,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Option<Index<'a>>> {
    let rows = match target {
        CatalogTarget::Family => return Ok(None),
        CatalogTarget::ShieldGenerators => {
            coverage::component_projections(catalog, SHIELD_GENERATORS, cancelled)?
        }
        CatalogTarget::WeaponFireActions { source_type } => {
            let components = coverage::component_projections(catalog, "weaponActions", cancelled)?;
            let mut actions = Index::new();
            for (owner, rows) in components {
                for component in rows {
                    error::check(cancelled)?;
                    for action in fire_actions(component, cancelled)? {
                        if action.pointer("/sourceType/value").and_then(Value::as_str)
                            == Some(source_type.as_str())
                        {
                            actions.entry(owner).or_default().push(action);
                        }
                    }
                }
            }
            actions
        }
    };
    for owner in rows.keys() {
        error::check(cancelled)?;
        if !owners.contains_key(owner) {
            return Err(Error::new(Kind::Identity, "canonical component has no Item owner").into());
        }
    }
    Ok(Some(rows))
}

fn fire_actions<'a>(
    component: &'a Value,
    cancelled: &impl Fn() -> bool,
) -> super::Result<Vec<&'a Value>> {
    let slots = match component
        .pointer("/fireActions/state")
        .and_then(Value::as_str)
    {
        Some("absent" | "null") => return Ok(Vec::new()),
        Some("present") => component
            .pointer("/fireActions/value")
            .and_then(Value::as_array),
        _ => None,
    }
    .ok_or_else(|| Error::new(Kind::Input, "unreadable authored fire-action slots"))?;
    let nodes = component
        .get("actionNodes")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::new(Kind::Input, "missing canonical action node table"))?;
    let mut actions = Vec::new();
    for slot in slots {
        error::check(cancelled)?;
        let position = match slot.get("state").and_then(Value::as_str) {
            Some("absent" | "null") => continue,
            Some("present") => slot
                .get("value")
                .and_then(Value::as_u64)
                .and_then(|index| usize::try_from(index).ok()),
            _ => None,
        };
        let node = position
            .and_then(|position| nodes.get(position))
            .ok_or_else(|| Error::new(Kind::Input, "invalid authored fire-action node index"))?;
        if node.pointer("/sourceType/state").and_then(Value::as_str) != Some("present")
            || node
                .pointer("/sourceType/value")
                .and_then(Value::as_str)
                .is_none()
        {
            return Err(Error::new(Kind::Input, "missing authored fire-action source type").into());
        }
        actions.push(node);
    }
    Ok(actions)
}
