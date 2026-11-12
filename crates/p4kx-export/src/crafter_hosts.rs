// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::contract::{Crafter, Loadout};
use crate::contract_loadout::{EntityKind, LoadoutPort};
use crate::guid_text::CatalogGuid;

/// Fills `host_guids` of every crafter in `crafters` with the ships whose Default Loadout mounts
/// it, at any depth of the port tree. `loadouts` is ordered by entity GUID, so the hosts come out
/// ordered.
pub(crate) fn link_hosts(crafters: &mut [Crafter<'_>], loadouts: &[Loadout]) {
    let index: HashMap<CatalogGuid, usize> = crafters
        .iter()
        .enumerate()
        .map(|(at, crafter)| (crafter.guid, at))
        .collect();
    if index.is_empty() {
        return;
    }
    for loadout in loadouts
        .iter()
        .filter(|loadout| loadout.entity_kind == EntityKind::Ship)
    {
        mounted(&loadout.ports, &mut |guid| {
            if let Some(&at) = index.get(&guid)
                && let Some(crafter) = crafters.get_mut(at)
                && crafter.host_guids.last() != Some(&loadout.entity_guid)
            {
                crafter.host_guids.push(loadout.entity_guid);
            }
        });
    }
}

fn mounted(ports: &[LoadoutPort], visit: &mut impl FnMut(CatalogGuid)) {
    for port in ports {
        if let Some(guid) = port.item_guid {
            visit(guid);
        }
        mounted(&port.ports, visit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use p4kx_extraction::Guid;

    fn guid(last: u128) -> CatalogGuid {
        CatalogGuid(Guid::from_stored(last.to_le_bytes()))
    }

    fn crafter(last: u128) -> Crafter<'static> {
        Crafter {
            guid: guid(last),
            class_name: "crafter",
            record_path: "path",
            r#type: None,
            sub_type: None,
            size: None,
            max_jobs_in_history: None,
            base_crafting_time: None,
            queues: Vec::new(),
            modifiers: Vec::new(),
            issues: Vec::new(),
            host_guids: Vec::new(),
        }
    }

    fn loadout(host: u128, kind: EntityKind, ports: Vec<LoadoutPort>) -> Loadout {
        Loadout {
            root_record_index: 0,
            port_inventory_complete: true,
            entity_guid: guid(host),
            entity_kind: kind,
            issues: Vec::new(),
            ports,
        }
    }

    fn port(item: u128, ports: Vec<LoadoutPort>) -> LoadoutPort {
        LoadoutPort {
            item_guid: Some(guid(item)),
            ports,
            ..LoadoutPort::default()
        }
    }

    #[test]
    fn a_ship_hosts_a_crafter_mounted_at_any_depth_once() {
        let mut crafters = [crafter(1), crafter(2)];
        let loadouts = [
            loadout(
                10,
                EntityKind::Ship,
                vec![port(9, vec![port(1, Vec::new())])],
            ),
            loadout(
                11,
                EntityKind::Ship,
                vec![port(1, Vec::new()), port(1, Vec::new())],
            ),
            loadout(12, EntityKind::Item, vec![port(1, Vec::new())]),
        ];
        link_hosts(&mut crafters, &loadouts);
        assert_eq!(crafters[0].host_guids, [guid(10), guid(11)]);
        assert!(crafters[1].host_guids.is_empty(), "a station has no host");
    }
}
