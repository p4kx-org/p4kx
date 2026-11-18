// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::loadout::alignment::LoadoutAlignment;

/// Which kind of entry a loadout belongs to; serialised as the strings `ship` and `item`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum EntityKind {
    /// An entry of `ships`.
    Ship,
    /// An entry of `items`.
    Item,
}

/// The ports of a ship or of an item that has ports, and what the Default Loadout puts into them.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct Loadout {
    #[serde(skip)]
    pub(crate) root_record_index: u32,
    /// The actual original physical Record position.
    pub(crate) source_record_index: SourcedValue<u32>,
    #[serde(skip)]
    pub(crate) port_inventory_complete: bool,
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
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LoadoutPort {
    #[serde(skip)]
    pub(crate) name_proof: Option<crate::catalog_read::original::AuthoredEntryPath>,
    #[serde(skip)]
    pub(crate) attachment_port_inventory_complete: bool,
    /// The port name, as the Loadout or the port definition spells it.
    pub(crate) name: SourcedValue<String>,
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
pub struct PortType {
    /// The item type, e.g. `WeaponGun`.
    pub(crate) r#type: String,
    /// The item subtypes the port takes; empty when it takes every subtype or the data gives
    /// none.
    pub(crate) sub_types: Vec<String>,
}

impl Loadout {
    /// Returns the physical original Record position.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.root_record_index
    }

    /// Borrows the physical Record position with its immediate original evidence.
    #[must_use]
    pub const fn source_record_index(&self) -> &SourcedValue<u32> {
        &self.source_record_index
    }

    /// Returns the original owning GUID.
    #[must_use]
    pub const fn entity_guid(&self) -> p4kx_extraction::Guid {
        self.entity_guid.get()
    }

    /// Returns the kind of catalog owner.
    #[must_use]
    pub const fn entity_kind(&self) -> EntityKind {
        self.entity_kind
    }

    /// Borrows the sorted resolver issue codes.
    #[must_use]
    pub fn issues(&self) -> &[&'static str] {
        &self.issues
    }

    /// Borrows the root ports in resolver order.
    #[must_use]
    pub fn ports(&self) -> &[LoadoutPort] {
        &self.ports
    }

    /// Looks up a port by its zero-based occurrence at each level.
    #[must_use]
    pub fn port(&self, occurrences: &[usize]) -> Option<&LoadoutPort> {
        let (&first, nested) = occurrences.split_first()?;
        nested
            .iter()
            .try_fold(self.ports.get(first)?, |port, &index| port.ports.get(index))
    }
}

impl LoadoutPort {
    pub(crate) fn named(name: SourcedValue<String>) -> Self {
        Self {
            name,
            name_proof: None,
            attachment_port_inventory_complete: false,
            source: "",
            min_size: None,
            max_size: None,
            types: Vec::new(),
            flags: Vec::new(),
            port_tags: Vec::new(),
            required_tags: Vec::new(),
            display_name_key: None,
            visibility: "",
            item_guid: None,
            item_class_name: None,
            docked_vehicle: false,
            unresolved: None,
            source_constraints: LoadoutAlignment::default(),
            ports: Vec::new(),
        }
    }

    /// Borrows the resolved original port name and its immediate source evidence.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<String> {
        &self.name
    }

    /// Returns the canonical port source code.
    #[must_use]
    pub const fn source(&self) -> &'static str {
        self.source
    }

    /// Returns the declared minimum item size.
    #[must_use]
    pub const fn min_size(&self) -> Option<u32> {
        self.min_size
    }

    /// Returns the declared maximum item size.
    #[must_use]
    pub const fn max_size(&self) -> Option<u32> {
        self.max_size
    }

    /// Borrows the allowed original item types in declaration order.
    #[must_use]
    pub fn types(&self) -> &[PortType] {
        &self.types
    }

    /// Borrows the original port flags.
    #[must_use]
    pub fn flags(&self) -> &[String] {
        &self.flags
    }

    /// Borrows the tags supplied by the port.
    #[must_use]
    pub fn port_tags(&self) -> &[String] {
        &self.port_tags
    }

    /// Borrows the tags required of the attachment.
    #[must_use]
    pub fn required_tags(&self) -> &[String] {
        &self.required_tags
    }

    /// Borrows the original display name localization key.
    #[must_use]
    pub fn display_name_key(&self) -> Option<&str> {
        self.display_name_key.as_deref()
    }

    /// Returns the canonical visibility code.
    #[must_use]
    pub const fn visibility(&self) -> &'static str {
        self.visibility
    }

    /// Returns the attached or authored target GUID when available.
    #[must_use]
    pub fn item_guid(&self) -> Option<p4kx_extraction::Guid> {
        self.item_guid.map(CatalogGuid::get)
    }

    /// Borrows the attached or authored target class when available.
    #[must_use]
    pub fn item_class_name(&self) -> Option<&str> {
        self.item_class_name.as_deref()
    }

    /// Reports whether resolution reached a docked vehicle.
    #[must_use]
    pub const fn docked_vehicle(&self) -> bool {
        self.docked_vehicle
    }

    /// Returns the original resolver's unresolved-attachment code.
    #[must_use]
    pub const fn unresolved(&self) -> Option<&'static str> {
        self.unresolved
    }

    /// Borrows the original physical port and attachment links.
    #[must_use]
    pub const fn source_constraints(&self) -> &LoadoutAlignment {
        &self.source_constraints
    }

    /// Borrows nested occurrences in resolver order.
    #[must_use]
    pub fn ports(&self) -> &[Self] {
        &self.ports
    }
}

impl PortType {
    /// Borrows the original accepted item type.
    #[must_use]
    pub const fn item_type(&self) -> &str {
        self.r#type.as_str()
    }

    /// Borrows the original accepted item subtypes.
    #[must_use]
    pub fn sub_types(&self) -> &[String] {
        &self.sub_types
    }
}

#[cfg(test)]
pub(crate) fn fixture_record_index(index: u32) -> SourcedValue<u32> {
    use p4kx_extraction::{SourceUses, SourceValue};

    SourceUses::new().value(SourceValue::Present(index), &[], false)
}
