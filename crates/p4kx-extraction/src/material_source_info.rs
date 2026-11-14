// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Positional authored material XML and interpreted material source descriptions.

use std::borrow::Cow;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[path = "material_source_site.rs"]
mod site;
pub use self::site::MaterialSourceSite;

#[path = "material_source_field_origins.rs"]
mod field_origins;
pub use self::field_origins::{MaterialSourceEvidence, MaterialSourceOriginError};

use crate::material_xml_info::{MaterialElementInfo, MaterialParameterInfo};

/// Actual source encoding, separate from generated XML or rendering output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialEncodingInfo {
    /// Original UTF-8 XML tokens.
    Text,
    /// Binary XML strings and node-content declarations.
    CryXml,
    /// The reader supplies an unsupported encoding.
    Unsupported,
}

/// Structured source coverage boundary actually reached by the material reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialSourceLossInfoKind {
    /// Invalid binary string-table bytes were repaired.
    RepairedEncoding,
    /// Canonical authored element capacity was exhausted.
    Elements,
    /// Canonical authored syntax/text storage was exhausted.
    TextBytes,
    /// A populated child list reached the depth bound.
    Depth,
    /// The reader supplies an unsupported loss category.
    Unsupported,
}

/// First original element location for one actual coverage boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct MaterialSourceLossInfo {
    /// Observed boundary, independent of raw byte preservation.
    pub kind: MaterialSourceLossInfoKind,
    /// Original element ordinals from the root.
    pub indices: Vec<usize>,
}

/// Existing surface-reader disposition of an original submaterial slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialEntryDispositionInfo {
    /// An interpreted material.
    Material,
    /// An unknown slot rendered with the existing engine-default fallback.
    EngineDefault,
    /// The reader supplies an unsupported disposition.
    Unsupported,
}

/// One original submaterial position without renumbering unknown entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct MaterialSourceEntryInfo<'source> {
    /// Original slot position.
    pub ordinal: usize,
    /// Source-reader element name.
    pub element_name: Cow<'source, str>,
    /// Existing rendering fallback or interpreted material disposition.
    pub disposition: MaterialEntryDispositionInfo,
}

/// One bounded canonical authored material root, kept once per source library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(transform = crate::projection_schema::require_properties)]
#[non_exhaustive]
pub struct MaterialLibrarySourceInfo<'source> {
    /// Actual original source encoding.
    pub encoding: MaterialEncodingInfo,
    /// Retained root syntax, absent only with an explicit loss boundary.
    pub root: Option<MaterialElementInfo<'source>>,
    /// Actual structured loss categories and first source locations.
    pub losses: Vec<MaterialSourceLossInfo>,
    /// Original submaterial dispositions; empty for a single-material library.
    pub entries: Vec<MaterialSourceEntryInfo<'source>>,
}

/// Raw source fields for one interpreted material, independent of derived surface decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct MaterialAuthoredInfo<'source> {
    /// Existing uninterpreted attributes in source order.
    pub other_attributes: Vec<MaterialParameterInfo<'source>>,
    /// Ordered shader parameters, preserving malformed/nonfinite tokens as text.
    pub public_params: Vec<MaterialParameterInfo<'source>>,
    /// Layer declarations in source order, without implicit companion reads.
    pub layers: Vec<MaterialLayerSourceInfo<'source>>,
    /// Unknown blocks in source order, including their source syntax supplement.
    pub elements: Vec<MaterialElementInfo<'source>>,
    /// Actual per-material typed-reader truncation descriptions.
    pub truncated: Vec<Cow<'source, str>>,
    /// Original texture-slot declarations and modifiers.
    pub textures: Vec<MaterialTextureSourceInfo<'source>>,
}

/// One layer declaration without resolved or inherited surface data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct MaterialLayerSourceInfo<'source> {
    /// All original layer attributes, including `Path`/`Submtl`/`PaletteTint` tokens.
    pub attributes: Vec<MaterialParameterInfo<'source>>,
}

/// One original texture slot, including placeholders and modifier blocks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct MaterialTextureSourceInfo<'source> {
    /// Original map/slot declaration.
    pub map: Cow<'source, str>,
    /// Original filename or engine placeholder token.
    pub file: Cow<'source, str>,
    /// Other slot attributes in source order.
    pub attributes: Vec<MaterialParameterInfo<'source>>,
    /// All retained modifier/unknown child declarations.
    pub children: Vec<MaterialElementInfo<'source>>,
}
