// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourcedValue, is_placeholder_key};
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::loadout::alignment::LoadoutAlignment;
use crate::loadout_owner_value::LoadoutOwnerValue;

#[path = "loadout_sub_types.rs"]
mod sub_types;
pub(crate) use sub_types::PortSubTypes;

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
    /// The owning Record GUID used to join the Loadout to `ships` or `items`.
    #[schemars(with = "SourcedValue<CatalogGuid>")]
    pub(crate) entity_guid: LoadoutOwnerValue<CatalogGuid>,
    /// Which kind `entityGuid` is an entry of, `ship` or `item`; a closed set the game may
    /// extend, so the schema keeps a string.
    #[schemars(with = "SourcedValue<String>")]
    pub(crate) entity_kind: LoadoutOwnerValue<EntityKind>,
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
    /// The smallest item size the port takes and its original source.
    pub(crate) min_size: SourcedValue<u32>,
    /// The largest item size the port takes and its original source.
    pub(crate) max_size: SourcedValue<u32>,
    /// The item types the port takes, in stored order; empty when the data names none.
    pub(crate) types: SourcedValue<Vec<PortType>>,
    /// The flags split at white space without a leading `$`, and their shared source.
    pub(crate) flags: SourcedValue<Vec<String>>,
    /// The offered tag words in stored order and their shared source.
    pub(crate) port_tags: SourcedValue<Vec<String>>,
    /// The required tag words in stored order and their shared source.
    pub(crate) required_tags: SourcedValue<Vec<String>>,
    /// The original display name key and its source, including authored empty or placeholder keys.
    pub(crate) display_name_key: SourcedValue<String>,
    /// Whether the port shows its item: `visible` or `invisible` as the vehicle implementation's
    /// part of that name says; without such a part `invisible` when the port definition carries
    /// the flag `invisible`, else `unspecified`. A closed set the game may extend, so the schema
    /// keeps a string.
    pub(crate) visibility: SourcedValue<&'static str>,
    /// The `guid` of the entry in `items` (or, for a docked vehicle, in `ships`) the port holds
    /// by default; `null` for an empty port. For an attachment that cannot be resolved it is the
    /// GUID the Loadout names, which may belong to no entry, or `null` when the Loadout names a
    /// class name only.
    pub(crate) item_guid: SourcedValue<CatalogGuid>,
    /// The class name of the attached Entity, or the class name the Loadout names; `null` for an
    /// empty port or an unresolved Reference.
    pub(crate) item_class_name: SourcedValue<String>,
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
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct PortType {
    /// The item type, e.g. `WeaponGun`.
    pub(crate) r#type: SourcedValue<String>,
    /// The item subtypes the port takes; empty when it takes every subtype or the data gives
    /// none.
    #[schemars(with = "SourcedValue<Vec<SourcedValue<String>>>")]
    pub(crate) sub_types: SourcedValue<PortSubTypes>,
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
        self.entity_guid.value.get()
    }

    /// Returns the kind of catalog owner.
    #[must_use]
    pub const fn entity_kind(&self) -> EntityKind {
        self.entity_kind.value
    }

    /// Returns the original owning GUID with its immediate header origin.
    #[must_use]
    pub fn source_entity_guid(&self) -> SourcedValue<p4kx_extraction::Guid> {
        self.entity_guid.sourced().map(|guid| guid.get())
    }

    /// Returns the owner kind with its actual original selection inputs.
    #[must_use]
    pub fn source_entity_kind(&self) -> SourcedValue<EntityKind> {
        self.entity_kind.sourced().map(|kind| *kind)
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

#[path = "loadout_port_default.rs"]
mod port_default;

impl LoadoutPort {
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
        self.min_size.value().copied()
    }

    /// Returns the declared maximum item size.
    #[must_use]
    pub const fn max_size(&self) -> Option<u32> {
        self.max_size.value().copied()
    }

    /// Borrows the minimum item size with its original source and value state.
    #[must_use]
    pub const fn source_min_size(&self) -> &SourcedValue<u32> {
        &self.min_size
    }

    /// Borrows the maximum item size with its original source and value state.
    #[must_use]
    pub const fn source_max_size(&self) -> &SourcedValue<u32> {
        &self.max_size
    }

    /// Borrows the allowed original item types in declaration order.
    #[must_use]
    pub fn types(&self) -> &[PortType] {
        self.types.value().map_or(&[], Vec::as_slice)
    }

    /// Borrows the ordered item type declarations and their selection evidence.
    #[must_use]
    pub const fn source_types(&self) -> &SourcedValue<Vec<PortType>> {
        &self.types
    }

    /// Borrows the original port flags.
    #[must_use]
    pub fn flags(&self) -> &[String] {
        self.flags.value().map_or(&[], Vec::as_slice)
    }

    /// Borrows the tags supplied by the port.
    #[must_use]
    pub fn port_tags(&self) -> &[String] {
        self.port_tags.value().map_or(&[], Vec::as_slice)
    }

    /// Borrows the tags required of the attachment.
    #[must_use]
    pub fn required_tags(&self) -> &[String] {
        self.required_tags.value().map_or(&[], Vec::as_slice)
    }

    /// Borrows the flag list and its original attribute or field evidence.
    #[must_use]
    pub const fn source_flags(&self) -> &SourcedValue<Vec<String>> {
        &self.flags
    }

    /// Borrows the offered tags and their original attribute or field evidence.
    #[must_use]
    pub const fn source_port_tags(&self) -> &SourcedValue<Vec<String>> {
        &self.port_tags
    }

    /// Borrows the required tags and their original attribute or field evidence.
    #[must_use]
    pub const fn source_required_tags(&self) -> &SourcedValue<Vec<String>> {
        &self.required_tags
    }

    /// Borrows a nonempty display name localization key that is not a placeholder.
    #[must_use]
    pub fn display_name_key(&self) -> Option<&str> {
        self.display_name_key
            .value()
            .map(String::as_str)
            .filter(|key| !key.is_empty() && !is_placeholder_key(key))
    }

    /// Borrows the authored display name key with its original evidence and value state.
    #[must_use]
    pub const fn source_display_name_key(&self) -> &SourcedValue<String> {
        &self.display_name_key
    }

    /// Returns the canonical visibility code.
    #[must_use]
    pub const fn visibility(&self) -> &'static str {
        match self.visibility.value() {
            Some(value) => value,
            None => "",
        }
    }

    /// Borrows the resolved visibility and the evidence for its selection.
    #[must_use]
    pub const fn source_visibility(&self) -> &SourcedValue<&'static str> {
        &self.visibility
    }

    /// Returns the attached or authored target GUID when available.
    #[must_use]
    pub fn item_guid(&self) -> Option<p4kx_extraction::Guid> {
        self.item_guid.value().copied().map(CatalogGuid::get)
    }

    /// Borrows the attached or authored target class when available.
    #[must_use]
    pub fn item_class_name(&self) -> Option<&str> {
        self.item_class_name.value().map(String::as_str)
    }

    /// Borrows the attached GUID and its immediate source evidence.
    #[must_use]
    pub const fn source_item_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.item_guid
    }

    /// Borrows the attached class name and its immediate source evidence.
    #[must_use]
    pub const fn source_item_class_name(&self) -> &SourcedValue<String> {
        &self.item_class_name
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
        match self.r#type.value() {
            Some(value) => value.as_str(),
            None => "",
        }
    }

    /// Borrows the original accepted item subtypes.
    #[must_use]
    pub fn sub_types(&self) -> &[String] {
        self.sub_types.value().map_or(&[], PortSubTypes::values)
    }

    /// Borrows the accepted item type and the evidence for its selected declaration.
    #[must_use]
    pub const fn source_item_type(&self) -> &SourcedValue<String> {
        &self.r#type
    }

    /// Returns the authored collection with each accepted subtype and its evidence.
    #[must_use]
    pub fn source_sub_types(&self) -> SourcedValue<Vec<SourcedValue<&str>>> {
        let values = self.sub_types.value();
        self.sub_types
            .map_ref(|_| values.into_iter().flat_map(PortSubTypes::members).collect())
    }
}

#[cfg(test)]
pub(crate) fn fixture_record_index(index: u32) -> SourcedValue<u32> {
    use p4kx_extraction::{SourceUses, SourceValue};

    SourceUses::new().value(SourceValue::Present(index), &[], false)
}
