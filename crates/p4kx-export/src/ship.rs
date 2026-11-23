// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourcedValue, StructValue, Value};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projections::ship_base_facts::ShipBaseFacts;
use crate::catalog_v3::authored_fields::ship_source_read::ShipSourceRead;
use crate::contract::Ship;
use crate::guid_text::CatalogGuid;
use crate::loadout::ledger::ShipVehicleValues;
use crate::paint_port::{PaintPort, VehicleFacts};
use crate::playability_rules::Evidence;
use crate::record_fields::{component, elements, structure, text};
use crate::ship_base::ShipRead;
use crate::{CatalogError, ExportAccountingError, Playability};

#[path = "ship_source_identity.rs"]
mod identity;
pub(crate) use identity::ShipIdentity;
#[path = "ship_source_projection.rs"]
pub(crate) mod source_projection;
#[path = "ship_source_row.rs"]
mod source_row;
pub use source_row::ShipSource;

const PAINTS: &str = "Paints";
pub(crate) const VEHICLE_COMPONENT: &str = "VehicleComponentParams";

/// Returns the item ports of the Record tree `tree` that take paints: the `SItemPortDef` entries
/// of its `SItemPortContainerComponentParams` with a type `Paints`.
pub(crate) fn paint_ports(tree: &RecordTree<'_>) -> Vec<PaintPort> {
    let container = component(tree, "SItemPortContainerComponentParams");
    elements(container, "Ports")
        .iter()
        .filter_map(|port| match port {
            Value::Struct(port) if takes_paints(port) => Some(PaintPort::new(
                text(Some(port), "PortTags"),
                text(Some(port), "RequiredPortTags"),
            )),
            _ => None,
        })
        .collect()
}

/// Returns the words of the ship Record's `AttachDef.Tags` in lower case.
pub(crate) fn lowered_tags(tree: &RecordTree<'_>) -> Vec<String> {
    let definition = structure(component(tree, "SAttachableComponentParams"), "AttachDef");
    text(definition, "Tags")
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect()
}

pub(crate) fn takes_paints(port: &StructValue<'_>) -> bool {
    elements(Some(port), "Types").iter().any(|kind| match kind {
        Value::Struct(kind) => {
            text(Some(kind), "Type").is_some_and(|name| name.eq_ignore_ascii_case(PAINTS))
        }
        _ => false,
    })
}

/// Projects original Ship fields together with the existing base, playability and paint results.
pub(crate) fn ship<'f>(
    tree: &RecordTree<'f>,
    source: ShipSourceRead<'_, 'f>,
    evidence: &Evidence,
    input: &OriginalCatalogInput<'_, '_>,
    read: &ShipRead,
    aggregates: crate::loadout::ledger::ShipAggregateValues,
) -> Result<(Ship<'f>, VehicleFacts, ShipBaseFacts), CatalogError> {
    let playability = source
        .playability_proof
        .decide(tree, source.vehicle, evidence, input)?;
    let base_facts = ShipBaseFacts::new(&source.header, aggregates.cargo);
    let ship = Ship::from_read(source, aggregates.vehicle, playability)?;
    let guid = CatalogGuid(tree.guid());
    let facts = VehicleFacts {
        source_record_index: Some(tree.record_index()),
        ship: guid,
        tags: lowered_tags(tree),
        paint_ports: paint_ports(tree),
        implementation: read.paint_ports.clone(),
    };
    Ok((ship, facts, base_facts))
}

impl<'source> Ship<'source> {
    pub(crate) fn from_read(
        source: ShipSourceRead<'_, 'source>,
        vehicle: ShipVehicleValues,
        playability: SourcedValue<Playability>,
    ) -> Result<Self, CatalogError> {
        let header = source.header;
        let identity = ShipIdentity::read(&header)?;
        let source_record_index = header
            .record_index()
            .value()
            .copied()
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        let presentation = source.presentation;
        let vehicle_kind = source
            .playability_proof
            .exosuit_kind()
            .unwrap_or(vehicle.vehicle_kind);
        Ok(Self {
            source_record_index,
            header,
            identity,
            source_numbers: source.numbers,
            name: presentation.name,
            description: presentation.description,
            role: presentation.role,
            career: presentation.career,
            crew_size: source.crew_size,
            vehicle_manufacturer: presentation.vehicle_manufacturer,
            attachment_manufacturer: presentation.attachment_manufacturer,
            vehicle_definition: source.vehicle_definition,
            playability,
            vehicle_kind,
            size: presentation.size,
            mass: vehicle.mass,
            mass_basis: vehicle.mass_basis,
            hull_health: vehicle.hull_health,
            variant_of: source.unchecked_variant,
            insurance: source.insurance,
            playability_inputs: source.playability_inputs,
        })
    }
}
