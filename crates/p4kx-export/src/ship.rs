// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue, Value};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_projections::ship_base_facts::ShipBaseFacts;
use crate::catalog_v3::authored_fields::ship_source_read::ShipSourceRead;
use crate::contract::Ship;
use crate::guid_text::CatalogGuid;
use crate::paint_port::{PaintPort, VehicleFacts};
use crate::playability_rules::Evidence;
use crate::record_fields::{component, elements, structure, text};
use crate::ship_base::ShipRead;
use crate::variants::ShipKey;

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
fn paint_ports(tree: &RecordTree<'_>) -> Vec<PaintPort> {
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
fn lowered_tags(tree: &RecordTree<'_>) -> Vec<String> {
    let definition = structure(component(tree, "SAttachableComponentParams"), "AttachDef");
    text(definition, "Tags")
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect()
}

fn takes_paints(port: &StructValue<'_>) -> bool {
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
    mut source: ShipSourceRead<'_, 'f>,
    evidence: &Evidence,
    input: &OriginalCatalogInput<'_, '_>,
    read: &ShipRead,
    aggregates: crate::loadout::ledger::ShipAggregateValues,
) -> Result<(Ship<'f>, VehicleFacts, ShipBaseFacts), CatalogError> {
    let identity = ShipIdentity::read(&source.header)?;
    let playability = source
        .playability_proof
        .decide(tree, source.vehicle, evidence, input)?;
    let source_numbers = std::mem::take(&mut source.numbers);
    let source = ShipSource::from_read(source, aggregates, playability);
    let base_facts = source.base_facts;
    let guid = CatalogGuid(tree.guid());
    let ship = Ship {
        source_record_index: tree.record_index(),
        header: source.header,
        identity,
        source_numbers,
        name: source.name,
        description: source.description,
        role: source.role,
        career: source.career,
        crew_size: source.crew_size,
        vehicle_manufacturer: source.vehicle_manufacturer,
        attachment_manufacturer: source.attachment_manufacturer,
        vehicle_definition: source.vehicle_definition,
        playability: source.playability,
        vehicle_kind: source.vehicle_kind,
        size: source.size,
        mass: source.mass,
        mass_basis: source.mass_basis,
        hull_health: source.hull_health,
        variant_of: None,
        insurance: source.insurance,
        playability_inputs: source.playability_inputs,
    };
    let facts = VehicleFacts {
        source_record_index: Some(tree.record_index()),
        ship: guid,
        tags: lowered_tags(tree),
        paint_ports: paint_ports(tree),
        implementation: read.paint_ports.clone(),
        variant_key: variant_key(tree, read),
    };
    Ok((ship, facts, base_facts))
}

/// Returns what the ship `tree` shares with the ships of its variant group: its vehicle
/// definition and the digest of its vehicle implementation, `None` when either is missing.
pub(crate) fn variant_key(tree: &RecordTree<'_>, read: &ShipRead) -> Option<ShipKey> {
    let definition = text(component(tree, VEHICLE_COMPONENT), "vehicleDefinition")?;
    Some(ShipKey::new(definition, read.implementation?))
}
