// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_dataforge::Guid;
use p4kx_extraction::{
    EntryOrigin, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError, GlbMeshOrigins,
    OriginArtifactBinding, OriginOutputAddress, SourceEvidenceError, SourceInventory,
    SourceLocation, SourceUses,
};
use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use crate::{Assembly, Part};

#[path = "paint_provenance_capture.rs"]
mod capture;

const GLB_ASSET_KEY: &str = "p4kxPaintAppearance";

pub(crate) fn glb_paint_extras(appearance: &PaintAppearanceValues) -> serde_json::Value {
    serde_json::json!({ (GLB_ASSET_KEY): appearance })
}

fn glb_paint_path(path: &FieldPath) -> Result<FieldPath, FieldPathError> {
    FieldPath::parse(&format!("/asset/extras/{GLB_ASSET_KEY}{}", path.as_str()))
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MaterialPreference {
    Geometry,
    CharacterDefinition,
    Model,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaintSelection {
    pub(crate) guid: Guid,
    pub(crate) record: Option<u32>,
    pub(crate) attachable: Option<usize>,
    pub(crate) tag: String,
    pub(crate) variant: VariantSource,
    pub(crate) material_preference: MaterialPreference,
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
                let path = glb_paint_path(path).map_err(io::Error::other)?;
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
        let Some((selection, part)) = self.paint_inputs() else {
            return Ok(None);
        };
        let uses = SourceUses::with_compact_use_refs();
        let Some((appearance, field_origins)) = capture::capture(self, selection, part, &uses)?
        else {
            return Ok(None);
        };
        Ok(Some(PaintProvenance {
            assembly: self,
            appearance,
            field_origins,
            source_inventory: uses.freeze()?,
        }))
    }

    /// Captures the painted asset fields before registering emitted meshes.
    ///
    /// # Errors
    /// Returns an error if the retained paint evidence cannot be represented.
    pub fn observe_glb_paint(
        &self,
        origins: &mut GlbMeshOrigins,
    ) -> Result<Option<PaintAppearanceValues>, PaintProvenanceError> {
        let Some((selection, part)) = self.paint_inputs() else {
            return Ok(None);
        };
        let Some((appearance, fields)) =
            capture::capture(self, selection, part, origins.source_uses())?
        else {
            return Ok(None);
        };
        let addressed = fields
            .iter()
            .map(|(path, origin)| Ok((glb_paint_path(path)?, origin.clone())))
            .collect::<Result<Vec<_>, FieldPathError>>()?;
        origins.include_json_fields(addressed);
        Ok(Some(appearance))
    }

    fn paint_inputs(&self) -> Option<(&PaintSelection, &Part)> {
        let root = self.root().filter(|part| part.model().is_some());
        self.paint.as_deref().zip(root)
    }
}
