// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::Guid;
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::template_property::PropertyValue;

/// The authored source level of a Generator property array.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum MissionPropertyLevel {
    /// The handler's `contractParams`.
    Handler,
    /// The contract's `paramOverrides`.
    Contract,
    /// The sub-contract's `paramOverrides` or direct fallback declarations.
    SubContract,
}

/// A mission property's authored declaration within one canonical source set.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionPropertyDeclaration<'f> {
    /// The exact source `missionVariableName`, including an empty string.
    pub(crate) variable: Option<String>,
    /// The exact source `extendedTextToken`, including an empty string.
    pub(crate) token: Option<String>,
    /// The concrete source value type without its `MissionPropertyValue_` prefix.
    pub(crate) value_type: Option<String>,
    /// The complete typed value, null for an absent or null value pointer.
    pub(crate) value: Option<PropertyValue<'f>>,
}

/// One authored `propertyOverrides` array, identified independently of its mutable payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct MissionPropertySet<'f> {
    /// The stable identifier derived from source Record GUID and authored location.
    pub(crate) guid: CatalogGuid,
    /// The original owning Record GUID.
    pub(crate) record_guid: CatalogGuid,
    /// The original owning Record path, without .xml.
    pub(crate) record_path: &'f str,
    /// The original owning Record struct.
    pub(crate) source_type: &'f str,
    /// The canonical location of the authored `propertyOverrides` array within its Record.
    pub(crate) source_location: String,
    /// The authoring level, without resolving which declaration wins at runtime.
    pub(crate) level: MissionPropertyLevel,
    /// Declarations in source order, retaining null pointer elements.
    pub(crate) properties: Vec<Option<MissionPropertyDeclaration<'f>>>,
}

/// Distinct source sets emitted by the selected Generator Records.
#[derive(Debug, Default)]
pub(crate) struct MissionPropertySets<'f>(BTreeMap<Guid, MissionPropertySet<'f>>);

impl<'f> MissionPropertySets<'f> {
    /// Returns whether this authored source identity is registered.
    pub(crate) fn contains(&self, guid: CatalogGuid) -> bool {
        self.0.contains_key(&guid.0)
    }

    /// Registers the parsed declaration once for its source identity.
    pub(crate) fn insert(&mut self, set: MissionPropertySet<'f>) {
        self.0.entry(set.guid.0).or_insert(set);
    }

    /// Combines disjoint owning Records and shared references to their source sets.
    pub(crate) fn merge(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    /// Returns the canonical set rows in GUID text order.
    pub(crate) fn into_rows(self) -> Vec<MissionPropertySet<'f>> {
        let mut rows: Vec<_> = self.0.into_values().collect();
        sort_by_guid(&mut rows, |set| set.guid);
        rows
    }
}
