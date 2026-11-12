// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_assembly::{
    Attachment, EntityNode, LoadoutError, PortDefinition, PortNode, PortSource, Target,
    VehicleItemPort,
};
use p4kx_extraction::is_placeholder_key;

use crate::CatalogError;
use crate::catalog_v3::catalog_item_alignment::AlignmentIndex;
use crate::contract_loadout::{LoadoutPort, PortType};
use crate::guid_text::CatalogGuid;
use crate::loadout::{alignment::LoadoutAlignment, ledger::Capture};

const VEHICLE_SOURCE: &str = "vehicle";

/// Turns the tree of an Entity into catalog ports and collects the codes of the issues found.
pub(crate) struct PortReader<'rows> {
    issues: BTreeSet<&'static str>,
    alignment: Option<&'rows AlignmentIndex<'rows>>,
}

impl<'rows> PortReader<'rows> {
    #[cfg(test)]
    pub(crate) const fn new() -> Self {
        Self {
            issues: BTreeSet::new(),
            alignment: None,
        }
    }

    pub(crate) const fn with_alignment(alignment: &'rows AlignmentIndex<'rows>) -> Self {
        Self {
            issues: BTreeSet::new(),
            alignment: Some(alignment),
        }
    }

    /// Returns the codes of the issues found so far, sorted, and forgets them.
    pub(crate) fn take_issues(&mut self) -> Vec<&'static str> {
        std::mem::take(&mut self.issues).into_iter().collect()
    }

    /// Returns the ports of `node` as catalog ports: the ports of its tree, which ends with the
    /// item ports only its vehicle implementation declares, listed with the source `vehicle`.
    /// Parts sharing a name all take the item port of the first, as the tree does.
    pub(crate) fn ports(
        &mut self,
        node: &EntityNode,
        capture: &Capture<'_>,
    ) -> Result<Vec<LoadoutPort>, CatalogError> {
        self.issues
            .extend(node.issues().iter().map(LoadoutError::kind));
        let vehicle = node.vehicle();
        node.ports()
            .iter()
            .map(|port| {
                let xml = vehicle.and_then(|vehicle| vehicle.item_port(port.name()));
                self.read_port(port, xml, capture)
            })
            .collect()
    }

    fn read_port(
        &mut self,
        port: &PortNode,
        xml: Option<&VehicleItemPort>,
        capture: &Capture<'_>,
    ) -> Result<LoadoutPort, CatalogError> {
        let shape = match (port.definition(), xml) {
            (Some(definition), _) => read_definition(definition),
            (None, Some(xml)) => read_vehicle(xml),
            (None, None) => LoadoutPort::default(),
        };
        let (source, visibility) = match (port.source(), xml) {
            (PortSource::Empty, Some(own)) => (VEHICLE_SOURCE, own.visibility().code()),
            (PortSource::Empty, None) => (VEHICLE_SOURCE, port.visibility().code()),
            (source, _) => (source.code(), port.visibility().code()),
        };
        let mut shape = LoadoutPort {
            name: port.name().to_owned(),
            name_proof: capture.port_name_proof(port.name(), port.name_proof())?,
            source,
            visibility,
            source_constraints: self.alignment.map_or_else(
                || Ok(LoadoutAlignment::default()),
                |index| LoadoutAlignment::read(port, index, capture),
            )?,
            ..shape
        };
        match port.attachment() {
            Attachment::Empty => {
                shape.ports = port
                    .nested()
                    .iter()
                    .map(|nested| self.read_port(nested, None, capture))
                    .collect::<Result<_, _>>()?;
            }
            Attachment::Entity(child) => {
                shape.item_guid = Some(CatalogGuid(child.guid()));
                shape.item_class_name = Some(child.class_name().to_owned());
                shape.docked_vehicle = child.is_docked_vehicle();
                shape.attachment_port_inventory_complete = child.issues().is_empty();
                if child.is_expanded() {
                    shape.ports = self.ports(child, capture)?;
                }
            }
            Attachment::Unresolved(unresolved) => {
                self.issues.insert(unresolved.error().kind());
                shape.unresolved = Some(unresolved.error().kind());
                match unresolved.target() {
                    Target::Reference(guid) => shape.item_guid = Some(CatalogGuid(*guid)),
                    Target::ClassName(name) => shape.item_class_name = Some(name.clone()),
                    Target::Unreadable => {}
                }
            }
        }
        Ok(shape)
    }
}

fn read_definition(definition: &PortDefinition) -> LoadoutPort {
    LoadoutPort {
        min_size: definition.min_size(),
        max_size: definition.max_size(),
        types: definition
            .types()
            .iter()
            .map(|kind| PortType {
                r#type: kind.name().to_owned(),
                sub_types: kind.sub_types().to_vec(),
            })
            .collect(),
        flags: words(definition.flags()),
        display_name_key: Some(definition.display_name())
            .filter(|key| !key.is_empty() && !is_placeholder_key(key))
            .map(str::to_owned),
        port_tags: tag_words(definition.port_tags()),
        required_tags: tag_words(definition.required_tags()),
        ..LoadoutPort::default()
    }
}

fn read_vehicle(port: &VehicleItemPort) -> LoadoutPort {
    LoadoutPort {
        min_size: port.min_size(),
        max_size: port.max_size(),
        types: port
            .types()
            .iter()
            .map(|kind| PortType {
                r#type: kind.clone(),
                sub_types: Vec::new(),
            })
            .collect(),
        flags: words(port.flags()),
        port_tags: tag_words(port.port_tags()),
        required_tags: tag_words(port.required_tags()),
        ..LoadoutPort::default()
    }
}

fn tag_words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

fn words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| word.trim_start_matches('$'))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}
