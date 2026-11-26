// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use super::cell::Cell;
use crate::contract_loadout::{EntityKind, Loadout, LoadoutPort, PortType};
use crate::guid_text::CatalogGuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ShipLoadout<'source> {
    source_record_index: Cell<&'source u32>,
    entity_guid: Cell<CatalogGuid>,
    entity_kind: Cell<EntityKind>,
    issues: &'source [&'static str],
    ports: Vec<Port<'source>>,
}

impl<'source> From<&'source Loadout> for ShipLoadout<'source> {
    fn from(loadout: &'source Loadout) -> Self {
        Self {
            source_record_index: loadout.source_record_index().into(),
            entity_guid: Cell::from(&loadout.source_entity_guid()).map(|guid| CatalogGuid(*guid)),
            entity_kind: Cell::from(&loadout.source_entity_kind()).map(|kind| *kind),
            issues: loadout.issues(),
            ports: loadout.ports().iter().map(Port::from).collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Port<'source> {
    name: Cell<&'source str>,
    display_name_key: Cell<&'source str>,
    min_size: Cell<&'source u32>,
    max_size: Cell<&'source u32>,
    types: Cell<Vec<AcceptedType<'source>>>,
    flags: Cell<&'source [String]>,
    port_tags: Cell<&'source [String]>,
    required_tags: Cell<&'source [String]>,
    item_guid: Cell<&'source CatalogGuid>,
    item_class_name: Cell<&'source str>,
    source: &'static str,
    unresolved: Option<&'static str>,
    ports: Vec<Self>,
}

impl<'source> From<&'source LoadoutPort> for Port<'source> {
    fn from(port: &'source LoadoutPort) -> Self {
        Self {
            name: Cell::from(port.name()).map(String::as_str),
            display_name_key: Cell::from(port.source_display_name_key()).map(String::as_str),
            min_size: port.source_min_size().into(),
            max_size: port.source_max_size().into(),
            types: Cell::from(port.source_types())
                .map(|types| types.iter().map(AcceptedType::from).collect()),
            flags: Cell::from(port.source_flags()).map(Vec::as_slice),
            port_tags: Cell::from(port.source_port_tags()).map(Vec::as_slice),
            required_tags: Cell::from(port.source_required_tags()).map(Vec::as_slice),
            item_guid: port.source_item_guid().into(),
            item_class_name: Cell::from(port.source_item_class_name()).map(String::as_str),
            source: port.source(),
            unresolved: port.unresolved(),
            ports: port.ports().iter().map(Self::from).collect(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AcceptedType<'source> {
    r#type: Cell<&'source str>,
    sub_types: Cell<Vec<Cell<&'source str>>>,
}

impl<'source> From<&'source PortType> for AcceptedType<'source> {
    fn from(port_type: &'source PortType) -> Self {
        Self {
            r#type: Cell::from(port_type.source_item_type()).map(String::as_str),
            sub_types: Cell::from(&port_type.source_sub_types()).map(|items| {
                items
                    .iter()
                    .map(|item| Cell::from(item).map(|value| *value))
                    .collect()
            }),
        }
    }
}
