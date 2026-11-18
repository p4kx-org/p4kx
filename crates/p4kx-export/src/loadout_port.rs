// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_assembly::{
    Attachment, EntityNode, LoadoutError, PortDefinition, PortNode, PortSource, Target,
    VehicleItemPort,
};
use p4kx_extraction::{SourceUse, SourcedValue, is_placeholder_key};

use crate::CatalogError;
use crate::catalog_read::original::AuthoredEntryPath;
use crate::contract_loadout::{LoadoutPort, PortType};
use crate::guid_text::CatalogGuid;
use crate::loadout::{
    alignment::{AlignmentLookup, LoadoutAlignment},
    ledger::Capture,
};

const VEHICLE_SOURCE: &str = "vehicle";

impl LoadoutPort {
    pub(crate) fn append_name_uses(&self, uses: &mut Vec<SourceUse>) -> Result<bool, CatalogError> {
        let Some(proof) = &self.name_proof else {
            return Ok(false);
        };
        let (original, complete) = proof.selector_inputs()?;
        uses.extend(original);
        Ok(complete)
    }
}

/// Turns the tree of an Entity into catalog ports and collects the codes of the issues found.
pub(crate) struct PortReader<'rows> {
    issues: BTreeSet<&'static str>,
    alignment: Option<&'rows mut dyn AlignmentLookup>,
}

impl<'rows> PortReader<'rows> {
    #[cfg(test)]
    pub(crate) const fn new() -> Self {
        Self {
            issues: BTreeSet::new(),
            alignment: None,
        }
    }

    pub(crate) const fn with_alignment(alignment: &'rows mut dyn AlignmentLookup) -> Self {
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
        let name_proof = capture.port_name_proof(port.name(), port.name_proof())?;
        let name = capture.port_name(port.name(), name_proof.as_ref())?;
        let shape = read_shape(port.definition(), xml, name, name_proof.as_ref(), capture)?;
        let (source, visibility) = match (port.source(), xml) {
            (PortSource::Empty, Some(own)) => (VEHICLE_SOURCE, own.visibility().code()),
            (PortSource::Empty, None) => (VEHICLE_SOURCE, port.visibility().code()),
            (source, _) => (source.code(), port.visibility().code()),
        };
        let mut shape = LoadoutPort {
            name_proof,
            source,
            visibility,
            source_constraints: self.alignment.as_deref_mut().map_or_else(
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

fn read_shape(
    definition: Option<&PortDefinition>,
    xml: Option<&VehicleItemPort>,
    name: SourcedValue<String>,
    proof: Option<&AuthoredEntryPath>,
    capture: &Capture<'_>,
) -> Result<LoadoutPort, CatalogError> {
    Ok(match (definition, xml) {
        (Some(definition), _) => {
            let (min_size, max_size) = capture.definition_sizes(definition, proof)?;
            LoadoutPort {
                min_size,
                max_size,
                ..read_definition(definition, name)
            }
        }
        (None, Some(xml)) => LoadoutPort {
            min_size: capture.vehicle_size(xml.min_size_source(), proof)?,
            max_size: capture.vehicle_size(xml.max_size_source(), proof)?,
            ..read_vehicle(xml, name)
        },
        (None, None) => LoadoutPort::named(name),
    })
}

fn read_definition(definition: &PortDefinition, name: SourcedValue<String>) -> LoadoutPort {
    LoadoutPort {
        types: definition
            .types()
            .iter()
            .map(|kind| PortType {
                r#type: kind.name().to_owned(),
                sub_types: kind.sub_types().to_vec(),
            })
            .collect(),
        flags: words(definition.flags()),
        display_name_key: display_name_key(Some(definition.display_name())),
        port_tags: tag_words(definition.port_tags()),
        required_tags: tag_words(definition.required_tags()),
        ..LoadoutPort::named(name)
    }
}

fn read_vehicle(port: &VehicleItemPort, name: SourcedValue<String>) -> LoadoutPort {
    LoadoutPort {
        types: port
            .item_types()
            .iter()
            .map(|kind| PortType {
                r#type: kind.item_type().to_owned(),
                sub_types: kind.subtypes().to_vec(),
            })
            .collect(),
        flags: words(port.flags()),
        display_name_key: display_name_key(port.display_name()),
        port_tags: tag_words(port.port_tags()),
        required_tags: tag_words(port.required_tags()),
        ..LoadoutPort::named(name)
    }
}

fn display_name_key(key: Option<&str>) -> Option<String> {
    key.filter(|key| !key.is_empty() && !is_placeholder_key(key))
        .map(str::to_owned)
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
