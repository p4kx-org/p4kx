// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::{MaterialAuthoredInfo, MaterialLibrarySourceInfo};
use schemars::JsonSchema;
use serde::Serialize;

use crate::material_reference::MaterialReference;
use crate::material_surface::MaterialSurface;

/// Built-in material payloads, separate from the common physical-source envelope.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "record_type", content = "payload", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialPayload<'source> {
    /// One source-library record including unused libraries and all original material slots.
    Library(&'source MaterialLibraryMetadata<'source>),
    /// Exact raw bytes survive a failed structured projection.
    Failure(MaterialProjectionFailure),
}

/// One physical material source with explicit shader-input receipts and no geometry conversion.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(extend("allOf" = [{"required": ["default_palette"]}]))]
#[non_exhaustive]
pub struct MaterialLibraryMetadata<'source> {
    /// Reader path declaration, without assuming that duplicate source paths are unique.
    pub path: Cow<'source, str>,
    /// Existing single-material versus submaterial-list meaning.
    pub single: bool,
    /// Original default-palette reference, without inherited values.
    pub default_palette: Option<Cow<'source, str>>,
    /// Existing number of unknown slots using the engine-default surface.
    pub unknown_entries: usize,
    /// Bounded authored root retained once, separate from every interpreted child.
    pub authored: MaterialLibrarySourceInfo<'source>,
    /// Original material positions and raw fields, including unknown fallback slots.
    pub materials: Vec<MaterialMetadata<'source>>,
    /// Ordered declared dependencies and honest common resolution states.
    pub references: Vec<MaterialReference>,
    /// Original shader/flag tokens and source-bound declaration links.
    pub shader_references: Vec<crate::material_shader_reference::MaterialShaderReference>,
    /// Whether every advertised structured field group was projected completely.
    pub projection_complete: bool,
}

/// One original material position with raw source and interpreted values kept distinct.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct MaterialMetadata<'source> {
    /// Original library material index, also zero for a single-material source.
    pub index: usize,
    /// Original material name, including an empty authored name.
    pub name: Cow<'source, str>,
    /// Raw fields through the common Extraction mapping seam.
    pub authored: MaterialAuthoredInfo<'source>,
    /// Existing reader interpretation, with explicit basis and separate raw XML tokens.
    pub interpreted_surface: MaterialSurface<'source>,
}

/// Stable structured-read failure without replacing its exact raw source artifact.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct MaterialProjectionFailure {
    /// Actual reader failure category.
    pub reason: MaterialFailureReason,
}

/// Material reader failure category, independent of Archive I/O or cancellation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialFailureReason {
    /// Plain XML contains invalid UTF-8 bytes.
    TextEncoding,
    /// Plain XML is malformed.
    Xml,
    /// Binary XML is malformed or the source encoding is unsupported.
    CryXml,
    /// Root is not a material declaration.
    NotMaterial,
    /// Plain XML exceeds its existing parser depth bound.
    XmlDepth,
    /// Structured input exceeds the configured byte bound.
    SourceBytes {
        /// Actual byte limit applied before the domain parse.
        limit: u64,
    },
    /// An existing typed material cardinality bound was reached.
    TypedLimit {
        /// Existing reader's static bounded collection name.
        what: String,
        /// Actual maximum allowed by that reader.
        limit: u64,
    },
    /// Validated XML source positions cannot be projected consistently.
    SourceSpan,
    /// Supplied reader limits are invalid.
    InvalidLimits,
    /// Reader supplies an unsupported failure variant.
    UnsupportedReader,
}
