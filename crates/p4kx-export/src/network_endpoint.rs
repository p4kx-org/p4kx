// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{
    SharedFieldOrigin, SourceUse, SourceUses, SourceValue, SourceValueState, SourcedValue, Step,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::catalog_item_alignment::{PhysicalPort, TopologyPortDefinition};
use crate::contract_loadout::LoadoutPort;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceWeakReference;
use crate::{CatalogError, ExportAccountingError};

/// One physical declaration or published Loadout occurrence matching an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct EndpointCandidate {
    loadout_port_ordinal: Option<u32>,
    physical_port: Option<PhysicalPort>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(super) enum NetworkEndpoint {
    Resolved { candidate: EndpointCandidate },
    Ambiguous { candidates: Vec<EndpointCandidate> },
    Unplaced { candidate: EndpointCandidate },
    Missing,
    Unavailable { candidates: Vec<EndpointCandidate> },
    Unspecified,
    OutsideDeclaringContext,
}

impl NetworkEndpoint {
    pub(super) fn port_ordinals(&self) -> impl Iterator<Item = u32> + '_ {
        let candidates = match self {
            Self::Resolved { candidate } => std::slice::from_ref(candidate),
            Self::Ambiguous { candidates } | Self::Unavailable { candidates } => candidates,
            _ => &[],
        };
        candidates
            .iter()
            .filter_map(|candidate| candidate.loadout_port_ordinal)
    }
}

/// The admissible endpoints among direct siblings of one installed Entity.
pub(super) struct LocalPorts<'scope, 'rows> {
    ports: &'rows [LoadoutPort],
    physical_uses: HashMap<PhysicalPort, &'rows SourceUse>,
    sources: &'scope SourceUses,
    origin: SharedFieldOrigin,
    by_name: HashMap<String, Vec<EndpointCandidate>>,
    by_physical_port: HashMap<PhysicalPort, Vec<EndpointCandidate>>,
    complete: bool,
}

impl<'scope, 'rows> LocalPorts<'scope, 'rows> {
    pub(super) fn new(
        ports: &'rows [LoadoutPort],
        definitions: &[TopologyPortDefinition<'rows>],
        inventory: &[&SourceUse],
        complete: bool,
        sources: &'scope SourceUses,
    ) -> Result<Self, CatalogError> {
        let origin = name_inventory_origin(sources, ports, definitions, inventory, complete)?;
        let physical_uses = definitions
            .iter()
            .filter_map(|definition| {
                definition
                    .name_use
                    .map(|original| (definition.position, original))
            })
            .collect();
        let mut local = Self {
            ports,
            physical_uses,
            sources,
            origin,
            by_name: HashMap::new(),
            by_physical_port: HashMap::new(),
            complete,
        };
        for (ordinal, port) in ports.iter().enumerate() {
            let ordinal =
                u32::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)?;
            let candidate = EndpointCandidate {
                loadout_port_ordinal: Some(ordinal),
                physical_port: port.source_constraints.physical_port()?,
            };
            local.add(&port.name, candidate);
        }
        for definition in definitions {
            if !local.by_physical_port.contains_key(&definition.position) {
                local.add(
                    definition.name,
                    EndpointCandidate {
                        loadout_port_ordinal: None,
                        physical_port: Some(definition.position),
                    },
                );
            }
        }
        Ok(local)
    }

    fn add(&mut self, name: &str, candidate: EndpointCandidate) {
        self.by_name
            .entry(p4kx_assembly::port_key(name))
            .or_default()
            .push(candidate);
        if let Some(physical) = candidate.physical_port {
            self.by_physical_port
                .entry(physical)
                .or_default()
                .push(candidate);
        }
    }

    pub(super) fn name(
        &self,
        source: &SourcedValue<&str>,
        original: Option<&SourceUse>,
    ) -> Result<SourcedValue<NetworkEndpoint>, CatalogError> {
        let result = self.resolve_name(source);
        if source.value().is_some_and(|name| !name.is_empty()) {
            self.bind_name(result, source, original)
        } else {
            Ok(self.bind_original(result, source, original))
        }
    }

    fn resolve_name(&self, source: &SourcedValue<&str>) -> NetworkEndpoint {
        let Some(name) = source.value().filter(|name| !name.is_empty()) else {
            return unavailable_name(source.state());
        };
        let candidates = self
            .by_name
            .get(&p4kx_assembly::port_key(name))
            .map_or(&[][..], Vec::as_slice);
        self.select(candidates)
    }

    pub(super) fn weak(
        &self,
        owner: u32,
        source: &SourcedValue<Box<SourceWeakReference<'_>>>,
        original: Option<&SourceUse>,
    ) -> Result<SourcedValue<NetworkEndpoint>, CatalogError> {
        let physical = match weak_target(owner, source) {
            Ok(physical) => physical,
            Err(result) => return Ok(self.bind_original(result, source, original)),
        };
        let Some(candidates) = self.by_physical_port.get(&physical) else {
            let result = NetworkEndpoint::Unavailable {
                candidates: vec![EndpointCandidate {
                    loadout_port_ordinal: None,
                    physical_port: Some(physical),
                }],
            };
            return Ok(self.sources.value(
                SourceValue::Present(result),
                original.map_or(&[], std::slice::from_ref),
                false,
            ));
        };
        let mut uses: Vec<_> = original.into_iter().cloned().collect();
        let mut complete = original.is_some() && source.origin().source_set_complete();
        if let Some(original) = self.physical_uses.get(&physical) {
            uses.push((*original).clone());
        } else {
            complete = false;
        }
        for ordinal in candidates
            .iter()
            .filter_map(|candidate| candidate.loadout_port_ordinal)
        {
            let ordinal =
                usize::try_from(ordinal).map_err(|_| ExportAccountingError::CounterOverflow)?;
            let port = self
                .ports
                .get(ordinal)
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
            complete &= append_name_uses(port, &mut uses)?;
        }
        let result = self.select(candidates);
        Ok(self.sources.value(
            SourceValue::Present(result),
            &uses,
            complete && self.complete,
        ))
    }

    fn bind_name<T>(
        &self,
        result: NetworkEndpoint,
        source: &SourcedValue<T>,
        original: Option<&SourceUse>,
    ) -> Result<SourcedValue<NetworkEndpoint>, CatalogError> {
        Ok(self.sources.value_with_shared_origin(
            SourceValue::Present(result),
            &self.origin,
            original.map_or(&[], std::slice::from_ref),
            original.is_some() && source.origin().source_set_complete(),
        )?)
    }

    fn bind_original<T>(
        &self,
        result: NetworkEndpoint,
        source: &SourcedValue<T>,
        original: Option<&SourceUse>,
    ) -> SourcedValue<NetworkEndpoint> {
        self.sources.value(
            SourceValue::Present(result),
            original.map_or(&[], std::slice::from_ref),
            original.is_some() && source.origin().source_set_complete(),
        )
    }

    fn select(&self, candidates: &[EndpointCandidate]) -> NetworkEndpoint {
        match candidates {
            candidates if !self.complete && candidates.len() <= 1 => NetworkEndpoint::Unavailable {
                candidates: candidates.to_vec(),
            },
            [candidate] if candidate.loadout_port_ordinal.is_some() => NetworkEndpoint::Resolved {
                candidate: *candidate,
            },
            [candidate] => NetworkEndpoint::Unplaced {
                candidate: *candidate,
            },
            candidates if !candidates.is_empty() => NetworkEndpoint::Ambiguous {
                candidates: candidates.to_vec(),
            },
            _ => NetworkEndpoint::Missing,
        }
    }
}

fn name_inventory_origin(
    sources: &SourceUses,
    ports: &[LoadoutPort],
    definitions: &[TopologyPortDefinition<'_>],
    inventory: &[&SourceUse],
    read_complete: bool,
) -> Result<SharedFieldOrigin, CatalogError> {
    let mut complete = read_complete;
    let mut uses: Vec<_> = inventory.iter().copied().cloned().collect();
    for port in ports {
        complete &= append_name_uses(port, &mut uses)?;
    }
    for definition in definitions {
        if let Some(original) = definition.name_use {
            uses.push(original.clone());
        } else {
            complete = false;
        }
    }
    Ok(sources.shared_origin(&uses, complete && !uses.is_empty()))
}

fn append_name_uses(port: &LoadoutPort, uses: &mut Vec<SourceUse>) -> Result<bool, CatalogError> {
    let Some(proof) = &port.name_proof else {
        return Ok(false);
    };
    let (original, complete) = proof.selector_inputs()?;
    uses.extend(original);
    Ok(complete)
}

fn weak_target(
    owner: u32,
    source: &SourcedValue<Box<SourceWeakReference<'_>>>,
) -> Result<PhysicalPort, NetworkEndpoint> {
    let pointer = source
        .value()
        .ok_or_else(|| unavailable_name(source.state()))?;
    let record = pointer
        .target_record_index
        .value()
        .copied()
        .ok_or_else(|| NetworkEndpoint::Unavailable {
            candidates: Vec::new(),
        })?;
    if record != owner {
        return Err(NetworkEndpoint::OutsideDeclaringContext);
    }
    let (component, port) = owned_port(pointer).ok_or(NetworkEndpoint::OutsideDeclaringContext)?;
    Ok(PhysicalPort {
        owner_record_index: record,
        component_ordinal: component,
        port_ordinal: port,
    })
}

fn owned_port(pointer: &SourceWeakReference<'_>) -> Option<(u32, u32)> {
    let [
        Step::Field("Components"),
        Step::Element(component),
        Step::Field("Ports"),
        Step::Element(port),
    ] = pointer.instance_steps()?
    else {
        return None;
    };
    Some((*component, *port))
}

const fn unavailable_name(state: SourceValueState) -> NetworkEndpoint {
    match state {
        SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null => {
            NetworkEndpoint::Unspecified
        }
        _ => NetworkEndpoint::Unavailable {
            candidates: Vec::new(),
        },
    }
}
