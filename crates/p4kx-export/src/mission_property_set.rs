// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "mission_property_set_rows/mod.rs"]
mod rows;
#[path = "mission_property_text.rs"]
mod text;

pub(crate) use rows::{MissionPropertySets, SetOwner};
pub use text::MissionPropertyText;

use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::template_property::PropertyValue;

/// The authored source level of a Generator property array.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::schema_vocabulary::open_string_vocabulary)]
#[non_exhaustive]
pub enum MissionPropertyLevel {
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
#[non_exhaustive]
pub struct MissionPropertyDeclaration<'f> {
    /// The exact source `missionVariableName`, including an empty string.
    pub variable: Option<String>,
    /// The exact source `extendedTextToken`, including an empty string.
    pub token: Option<String>,
    /// The concrete source value type without its `MissionPropertyValue_` prefix.
    pub value_type: Option<String>,
    /// The complete typed value, null for an absent or null value pointer.
    pub value: Option<PropertyValue<'f>>,
}

/// One authored `propertyOverrides` array, identified independently of its mutable payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct MissionPropertySet<'f> {
    /// The stable identifier derived from source Record GUID and authored location.
    pub guid: CatalogGuid,
    /// The original owning Record GUID.
    pub record_guid: CatalogGuid,
    /// The original owning Record path, without .xml.
    pub record_path: &'f str,
    /// The original owning Record struct.
    pub source_type: &'f str,
    /// The canonical location of the authored `propertyOverrides` array within its Record.
    pub source_location: String,
    /// The authoring level, without resolving which declaration wins at runtime.
    pub level: MissionPropertyLevel,
    /// Declarations in source order, retaining null pointer elements.
    pub properties: Vec<Option<MissionPropertyDeclaration<'f>>>,
}
