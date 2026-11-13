// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::MediaError;

/// Exact discriminator of the media contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum MediaFormat {
    /// The media export envelope.
    #[serde(rename = "p4kx-media")]
    P4kxMedia,
}

/// Exact media format version, independent of catalog schema versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub enum MediaVersion {
    /// Version 1.0 of the media index and record envelopes.
    #[serde(rename = "1.0")]
    V1,
}

/// Returns the JSON Schema of the media index envelope.
#[must_use]
pub fn index_schema() -> Value {
    schemars::schema_for!(crate::Index).to_value()
}

/// Returns the JSON Schema of one media metadata record.
#[must_use]
pub fn record_schema() -> Value {
    schemars::schema_for!(crate::MediaRecord).to_value()
}

/// Returns the record envelope with namespace-specific built-in payload constraints.
/// Custom provider namespaces retain the generic record data contract.
///
/// # Errors
/// Returns incompatible generated schema definitions or structural schema failures.
pub fn builtin_record_schema() -> Result<Value, MediaError> {
    let shader = crate::shader_schema::bind_record_schema(record_schema())?;
    let archive = crate::archive_metadata_schema::bind_record_schema(shader)?;
    let layout = crate::archive_layout_schema::bind_record_schema(archive)?;
    let material = crate::material_schema::bind_record_schema(layout)?;
    let audio = crate::audio_schema::bind_record_schema(material)?;
    let control = crate::audio_control_schema::bind_record_schema(audio)?;
    let animation = crate::animation::bind_record_schema(control)?;
    let animation = crate::animation_definition::bind_record_schema(animation)?;
    let texture = crate::texture_provider::schema::bind_record_schema(animation)?;
    crate::provider::model_source::bind_record_schema(texture)
}

/// Returns the JSON Schema of the common typed reference envelope.
#[must_use]
pub fn reference_schema() -> Value {
    schemars::schema_for!(crate::Reference).to_value()
}

/// Returns the JSON Schema of one retained source inventory line.
#[must_use]
pub fn source_schema() -> Value {
    schemars::schema_for!(crate::Source).to_value()
}

pub(crate) fn reference_definition(_: &mut SchemaGenerator) -> Schema {
    schemars::json_schema!({"$ref": "#/$defs/Reference"})
}
