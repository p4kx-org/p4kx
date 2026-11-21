// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::merge::port_key;
use crate::port::PortDefinition;
use crate::resolver_observation::Observation;
use crate::spec::{EntitySpec, EntrySpec, LoadoutOrigin, LoadoutSpec, TargetSpec};
use crate::{
    LoadoutPath, LoadoutXmlClassUse, PortSource, RecordLoadoutEffect, RecordPortSelection,
    VehicleImplementation,
};

/// One port of an Entity before it is resolved.
pub(super) struct Pending<'s> {
    pub(super) name: &'s str,
    pub(super) name_proof: Option<&'s LoadoutPath>,
    pub(super) source: PortSource,
    pub(super) definition: Option<Arc<PortDefinition>>,
    pub(super) target: &'s TargetSpec,
    pub(super) loadout: Option<&'s LoadoutSpec>,
    pub(super) origin: Option<&'s LoadoutOrigin>,
    pub(super) xml_use: Option<&'s LoadoutXmlClassUse>,
}

/// Returns the ports of an Entity in order: the merged Loadout entries, then the defined ports
/// no entry names, then the item ports only the vehicle implementation declares, each part once
/// even where several share a name.
pub(super) fn pending_ports<'s>(
    spec: &'s EntitySpec,
    entries: &'s [(EntrySpec, PortSource)],
    vehicle: Option<&'s VehicleImplementation>,
    observation: Option<&'s Observation<'_>>,
) -> impl Iterator<Item = Pending<'s>> {
    let mut selected = observation.is_some().then(HashMap::new);
    let named: HashSet<String> = entries
        .iter()
        .map(|(entry, _)| {
            let key = port_key(&entry.port);
            if let Some(selected) = &mut selected {
                selected.entry(key.clone()).or_insert(entry);
            }
            key
        })
        .collect();
    let mut taken = named.clone();
    taken.extend(spec.ports.iter().map(|port| port_key(&port.name)));
    let declared = vehicle
        .into_iter()
        .flat_map(VehicleImplementation::item_ports)
        .filter(move |part| !taken.contains(&port_key(part.name())))
        .map(|part| Pending {
            name: part.name(),
            name_proof: part.name_proof.as_ref(),
            source: PortSource::Empty,
            definition: None,
            target: &TargetSpec::None,
            loadout: None,
            origin: None,
            xml_use: None,
        });
    let from_entries = entries.iter().map(|(entry, source)| Pending {
        name: &entry.port,
        name_proof: entry.port_proof.as_ref(),
        source: *source,
        definition: spec
            .port(&entry.port)
            .map(|port| Arc::clone(&port.definition)),
        target: &entry.target,
        loadout: entry.loadout.as_ref(),
        origin: Some(&entry.origin),
        xml_use: entry.xml_use.as_ref(),
    });
    let defined = defined_ports(spec, named, selected, observation);
    from_entries.chain(defined).chain(declared)
}

fn defined_ports<'s>(
    spec: &'s EntitySpec,
    named: HashSet<String>,
    selected: Option<HashMap<String, &'s EntrySpec>>,
    observation: Option<&'s Observation<'_>>,
) -> impl Iterator<Item = Pending<'s>> {
    spec.ports
        .iter()
        .filter(move |port| {
            let key = port_key(&port.name);
            if !named.contains(&key) {
                return true;
            }
            if port.default_item != TargetSpec::None
                && let (Some(observation), Some(selected), Some(owner_record_index)) =
                    (observation, selected.as_ref(), spec.record_index)
                && let Some(winner) = selected
                    .get(&key)
                    .and_then(|entry| RecordPortSelection::entry(entry))
            {
                observation.record_loadout_effect(&RecordLoadoutEffect::SuppressDefault {
                    owner_record_index,
                    declared: RecordPortSelection::declared(port),
                    winner,
                });
            }
            false
        })
        .map(|port| Pending {
            name: &port.name,
            name_proof: Some(&port.name_proof),
            source: if port.default_item == TargetSpec::None {
                PortSource::Unset
            } else {
                PortSource::PortDefault
            },
            definition: Some(Arc::clone(&port.definition)),
            target: &port.default_item,
            loadout: None,
            origin: (port.default_item != TargetSpec::None).then_some(&spec.origin),
            xml_use: None,
        })
}
