// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_dataforge::Guid;
use p4kx_extraction::{
    EntryOrigin, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError, OriginArtifactBinding,
    OriginOutputAddress, SourceEvidenceError, SourceInventory, SourceLocation,
};
use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use crate::Assembly;

#[path = "paint_provenance_capture.rs"]
mod capture;

pub(crate) const GLB_ASSET_KEY: &str = "p4kxPaintAppearance";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
enum VariantKind {
    SubGeometry,
    MaterialVariant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VariantSource {
    kind: VariantKind,
    location: Option<SourceLocation>,
    tag_locations: Box<[SourceLocation]>,
}

impl VariantSource {
    pub(crate) const fn geometry(
        location: Option<SourceLocation>,
        tag_locations: Box<[SourceLocation]>,
    ) -> Self {
        Self {
            kind: VariantKind::SubGeometry,
            location,
            tag_locations,
        }
    }

    pub(crate) const fn material(
        location: SourceLocation,
        tag_locations: Box<[SourceLocation]>,
    ) -> Self {
        Self {
            kind: VariantKind::MaterialVariant,
            location: Some(location),
            tag_locations,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaintSelection {
    pub(crate) guid: Guid,
    pub(crate) record: Option<u32>,
    pub(crate) attachable: Option<usize>,
    pub(crate) tag: String,
    pub(crate) variant: VariantSource,
    pub(crate) compatibility: Vec<PaintSelectionInput>,
}

#[derive(Debug, Clone)]
pub(crate) enum PaintSelectionInput {
    DataForge(SourceLocation),
    Xml(EntryOrigin, SourceLocation),
}

impl PartialEq for PaintSelectionInput {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::DataForge(left), Self::DataForge(right)) => left == right,
            (Self::Xml(left, location), Self::Xml(right, other_location)) => {
                left.binding().is_same_entry(&right.binding()) && location == other_location
            }
            _ => false,
        }
    }
}

impl Eq for PaintSelectionInput {}

/// Observed identity fields of the selected hull paint and its actual appearance.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaintAppearanceValues {
    #[serde(skip_serializing_if = "Option::is_none")]
    paint_guid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    appearance_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variant_kind: Option<VariantKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    geometry: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    material: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    palette_guid: Option<String>,
}

/// An appearance with field origins in a shared physical source inventory.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaintProvenance<'assembly> {
    #[serde(skip)]
    pub(crate) assembly: &'assembly Assembly,
    appearance: PaintAppearanceValues,
    field_origins: FieldOrigins,
    source_inventory: SourceInventory,
}

/// The selected paint's physical evidence could not be represented.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PaintProvenanceError {
    /// The physical inventory rejected the observed source.
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
    /// An appearance field path could not be represented.
    #[error(transparent)]
    FieldPath(#[from] FieldPathError),
    /// Several observations claim the same appearance field.
    #[error(transparent)]
    FieldOrigins(#[from] FieldOriginsError),
}

impl PaintProvenance<'_> {
    /// Returns the selected appearance fields written to both export formats.
    #[must_use]
    pub const fn appearance(&self) -> &PaintAppearanceValues {
        &self.appearance
    }

    /// Writes the physical origins of the GLB's selected paint fields.
    ///
    /// # Errors
    /// Returns an error when a field address or companion cannot be written.
    pub fn write_glb_companion(
        &self,
        writer: impl Write,
        binding: &OriginArtifactBinding,
    ) -> io::Result<()> {
        let fields: Vec<_> = self
            .field_origins
            .iter()
            .map(|(path, origin)| {
                let path =
                    FieldPath::parse(&format!("/asset/extras/{GLB_ASSET_KEY}{}", path.as_str()))
                        .map_err(io::Error::other)?;
                Ok((OriginOutputAddress::GlbJsonField { path }, origin))
            })
            .collect::<io::Result<_>>()?;
        p4kx_extraction::write_origin_companion(
            writer,
            binding,
            &self.source_inventory,
            fields.iter().map(|(address, origin)| (address, *origin)),
        )
    }
}

impl Assembly {
    /// Captures the rendered root's fitted paint with its observed origin fields.
    ///
    /// # Errors
    /// Returns an error if the retained evidence cannot be represented.
    pub fn paint_provenance(&self) -> Result<Option<PaintProvenance<'_>>, PaintProvenanceError> {
        let root = self.root().filter(|part| part.model().is_some());
        let Some((selection, part)) = self.paint.as_ref().zip(root) else {
            return Ok(None);
        };
        capture::capture(self, selection, part)
    }
}
