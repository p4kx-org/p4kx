// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "mission_property_set_rows/mod.rs"]
mod rows;

pub(crate) use rows::{MissionPropertySets, SetOwner};

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
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
