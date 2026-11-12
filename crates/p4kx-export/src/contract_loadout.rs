// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::loadout::alignment::LoadoutAlignment;

/// Which kind of entry a loadout belongs to; serialised as the strings `ship` and `item`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum EntityKind {
    /// An entry of `ships`.
    Ship,
    /// An entry of `items`.
    Item,
}

/// The ports of a ship or of an item that has ports, and what the Default Loadout puts into them.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Loadout {
    /// The `guid` of the entry in `ships` or `items` the ports belong to; loadouts are ordered by
    /// it.
    pub(crate) entity_guid: CatalogGuid,
    /// Which kind `entityGuid` is an entry of, `ship` or `item`; a closed set the game may
    /// extend, so the schema keeps a string.
    #[schemars(with = "String")]
    pub(crate) entity_kind: EntityKind,
    /// The codes of the problems that kept part of the Loadout from being read, e.g.
    /// `xml_loadout`, sorted and without duplicates; empty when everything was read.
    pub(crate) issues: Vec<&'static str>,
    /// The ports of the Entity: those a Loadout names in Loadout order, then the other defined
    /// ports in definition order, then the item ports of the vehicle implementation no Record
    /// port names.
    pub(crate) ports: Vec<LoadoutPort>,
}

/// One item port and what the Default Loadout attaches to it.
#[derive(Debug, Default, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LoadoutPort {
    /// The port name, as the Loadout or the port definition spells it.
    pub(crate) name: String,
    /// Where the attachment comes from: `loadout` (the Entity's own Default Loadout), `override`
    /// (the Loadout of the entry that attached the Entity), `port_default` (the default item of
    /// the port definition), `unset` (the port is defined and holds nothing) or `vehicle` (the
    /// port is known only from the vehicle implementation and holds nothing). A closed set the
    /// game may extend, so the schema keeps a string.
    pub(crate) source: &'static str,
    /// The smallest item size the port takes, `null` when the data gives none.
    pub(crate) min_size: Option<u32>,
    /// The largest item size the port takes, `null` when the data gives none.
    pub(crate) max_size: Option<u32>,
    /// The item types the port takes, in stored order; empty when the data names none.
    pub(crate) types: Vec<PortType>,
    /// The flags of the port as stored, split at white space, without a leading `$`, e.g.
    /// `invisible`, `uneditable`.
    pub(crate) flags: Vec<String>,
    /// The tags the port offers: the words of the stored value, split at white space and
    /// otherwise unchanged, like `tags` of an item.
    pub(crate) port_tags: Vec<String>,
    /// The tags an item must carry to fit the port: the words of the stored value, split at white
    /// space and otherwise unchanged, like `tags` of an item.
    pub(crate) required_tags: Vec<String>,
    /// The localisation key of the port's display name; `null` for none or a placeholder key.
    pub(crate) display_name_key: Option<String>,
    /// Whether the port shows its item: `visible` or `invisible` as the vehicle implementation's
    /// part of that name says; without such a part `invisible` when the port definition carries
    /// the flag `invisible`, else `unspecified`. A closed set the game may extend, so the schema
    /// keeps a string.
    pub(crate) visibility: &'static str,
    /// The `guid` of the entry in `items` (or, for a docked vehicle, in `ships`) the port holds
    /// by default; `null` for an empty port. For an attachment that cannot be resolved it is the
    /// GUID the Loadout names, which may belong to no entry, or `null` when the Loadout names a
    /// class name only.
    pub(crate) item_guid: Option<CatalogGuid>,
    /// The class name of the attached Entity, or the class name the Loadout names; `null` for an
    /// empty port or an unresolved Reference.
    pub(crate) item_class_name: Option<String>,
    /// Whether the attached Entity is a docked vehicle, e.g. a snub craft in a hangar port. Its
    /// ports are not listed.
    pub(crate) docked_vehicle: bool,
    /// The code of the error that kept the attachment from being resolved, e.g.
    /// `dangling_reference` or `unknown_class`; `null` otherwise.
    pub(crate) unresolved: Option<&'static str>,
    /// Physical source configuration links for the port definition and attached Entity.
    pub(crate) source_constraints: LoadoutAlignment,
    /// The ports of the attached Entity under this Loadout, or the ports of the nested Loadout of
    /// an empty port; empty for a port without either.
    pub(crate) ports: Vec<Self>,
}

/// An item type a port takes.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct PortType {
    /// The item type, e.g. `WeaponGun`.
    pub(crate) r#type: String,
    /// The item subtypes the port takes; empty when it takes every subtype or the data gives
    /// none.
    pub(crate) sub_types: Vec<String>,
}
