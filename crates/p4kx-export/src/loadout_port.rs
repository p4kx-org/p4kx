// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_assembly::{
    Attachment, EntityNode, LoadoutError, PortDefinition, PortNode, PortSource,
    VehiclePortSelection,
};
use p4kx_extraction::{Archive, SourcedValue};

use crate::CatalogError;
use crate::catalog_read::original::AuthoredEntryPath;
use crate::contract_loadout::LoadoutPort;
use crate::loadout::{
    alignment::{AlignmentLookup, LoadoutAlignment},
    ledger::Capture,
};

const VEHICLE_SOURCE: &str = "vehicle";

/// Turns the tree of an Entity into catalog ports and collects the codes of the issues found.
pub(crate) struct PortReader<'rows> {
    issues: BTreeSet<&'static str>,
    alignment: Option<&'rows mut dyn AlignmentLookup>,
    archive: Option<&'rows Archive>,
}

impl<'rows> PortReader<'rows> {
    #[cfg(test)]
    pub(crate) const fn new() -> Self {
        Self {
            issues: BTreeSet::new(),
            alignment: None,
            archive: None,
        }
    }

    pub(crate) const fn with_alignment(
        alignment: &'rows mut dyn AlignmentLookup,
        archive: &'rows Archive,
    ) -> Self {
        Self {
            issues: BTreeSet::new(),
            alignment: Some(alignment),
            archive: Some(archive),
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
                let xml = vehicle.and_then(|vehicle| vehicle.item_port_with_selection(port.name()));
                self.read_port(port, xml.as_ref(), node.record_index(), capture)
            })
            .collect()
    }

    fn read_port(
        &mut self,
        port: &PortNode,
        xml: Option<&VehiclePortSelection<'_>>,
        record_index: Option<u32>,
        capture: &Capture<'_>,
    ) -> Result<LoadoutPort, CatalogError> {
        let name_proof = capture.port_name_proof(port.name(), port.name_proof())?;
        let name = capture.port_name(port.name(), name_proof.as_ref())?;
        let shape = read_shape(
            port.definition(),
            xml,
            name,
            name_proof.as_ref(),
            capture,
            self.archive,
            record_index,
        )?;
        let (source, visibility_code) = match (port.source(), xml.map(VehiclePortSelection::port)) {
            (PortSource::Empty, Some(own)) => (VEHICLE_SOURCE, own.visibility().code()),
            (PortSource::Empty, None) => (VEHICLE_SOURCE, port.visibility().code()),
            (source, _) => (source.code(), port.visibility().code()),
        };
        let visibility = if shape.visibility.value().is_some() {
            shape.visibility
        } else {
            capture.port_visibility(
                visibility_code,
                xml,
                name_proof.as_ref(),
                port.definition().map(|_| &shape.flags),
                self.archive,
                record_index,
            )?
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
        self.read_attachment(port, &mut shape, capture)?;
        Ok(shape)
    }

    fn read_attachment(
        &mut self,
        port: &PortNode,
        shape: &mut LoadoutPort,
        capture: &Capture<'_>,
    ) -> Result<(), CatalogError> {
        match port.attachment() {
            Attachment::Empty => {
                shape.ports = port
                    .nested()
                    .iter()
                    .map(|nested| self.read_port(nested, None, None, capture))
                    .collect::<Result<_, _>>()?;
            }
            Attachment::Entity(child) => {
                let values = capture.attached_values(child)?;
                shape.item_guid = values.guid;
                shape.item_class_name = values.class_name;
                shape.docked_vehicle = child.is_docked_vehicle();
                shape.attachment_port_inventory_complete = child.issues().is_empty();
                if child.is_expanded() {
                    shape.ports = self.ports(child, capture)?;
                }
            }
            Attachment::Unresolved(unresolved) => {
                self.issues.insert(unresolved.error().kind());
                shape.unresolved = Some(unresolved.error().kind());
                let values = capture.unresolved_attachment_values(unresolved)?;
                shape.item_guid = values.guid;
                shape.item_class_name = values.class_name;
            }
        }
        Ok(())
    }
}

fn read_shape(
    definition: Option<&PortDefinition>,
    xml: Option<&VehiclePortSelection<'_>>,
    name: SourcedValue<String>,
    proof: Option<&AuthoredEntryPath>,
    capture: &Capture<'_>,
    archive: Option<&Archive>,
    record_index: Option<u32>,
) -> Result<LoadoutPort, CatalogError> {
    let attributes = match (definition, xml) {
        (Some(definition), _) => capture.definition_attributes(definition, proof)?,
        (None, Some(xml)) => capture.vehicle_attributes(xml, proof, archive, record_index)?,
        (None, None) => return Ok(LoadoutPort::named(name)),
    };
    let mut port = LoadoutPort {
        min_size: attributes.min_size,
        max_size: attributes.max_size,
        types: attributes.types,
        flags: attributes.flags,
        port_tags: attributes.port_tags,
        required_tags: attributes.required_tags,
        display_name_key: attributes.display_name_key,
        ..LoadoutPort::named(name)
    };
    if let Some(visibility) = attributes.visibility {
        port.visibility = visibility;
    }
    Ok(port)
}
