// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_dataforge::Guid;
use p4kx_extraction::{
    DataForgeOrigin, EntryOrigin, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError,
    InputScope, OriginArtifactBinding, OriginOutputAddress, SourceCoordinateOmission,
    SourceEvidenceError, SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses, directory_path_uses,
};
use schemars::JsonSchema;
use serde::Serialize;
use thiserror::Error;

use crate::{Assembly, PaletteSelection, PaletteSource, Part};

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
        let uses = SourceUses::with_compact_use_refs();
        let Some(forge) = forge_scope(&uses, self.forge_origin.as_deref())? else {
            return Ok(None);
        };
        let mut capture = Capture {
            uses: &uses,
            forge: &forge,
            fields: FieldOrigins::new(),
        };
        let appearance = capture.appearance(selection, part)?;
        Ok(Some(PaintProvenance {
            assembly: self,
            appearance,
            field_origins: capture.fields,
            source_inventory: uses.freeze()?,
        }))
    }
}

struct Capture<'capture> {
    uses: &'capture SourceUses,
    forge: &'capture SourceScope,
    fields: FieldOrigins,
}

impl Capture<'_> {
    fn appearance(
        &mut self,
        selection: &PaintSelection,
        part: &Part,
    ) -> Result<PaintAppearanceValues, PaintProvenanceError> {
        let paint_guid = selection
            .record
            .map(|record_index| {
                self.single(
                    "/paintGuid",
                    header(record_index, "guid"),
                    SourceUseRole::Payload,
                )?;
                Ok::<_, PaintProvenanceError>(selection.guid.to_string())
            })
            .transpose()?;
        let tag = selection
            .record
            .zip(selection.attachable)
            .map(|(record_index, component)| {
                self.forge.at(
                    SourceLocation::DataForge {
                        record_index,
                        field: format!("Components[{component}].AttachDef.Tags"),
                    },
                    SourceUseRole::Selector,
                )
            })
            .transpose()?;
        let appearance_tag = if let Some(tag) = tag.as_ref() {
            self.record("/appearanceTag", std::slice::from_ref(tag), true)?;
            Some(selection.tag.clone())
        } else {
            None
        };
        let variant_kind = self.variant(selection, tag)?;
        let geometry = part
            .geometry()
            .zip(part.geometry_selector())
            .map(|(path, location)| {
                self.single("/geometry", location, SourceUseRole::Selector)?;
                Ok::<_, PaintProvenanceError>(path.to_owned())
            })
            .transpose()?;
        Ok(PaintAppearanceValues {
            paint_guid,
            appearance_tag,
            variant_kind,
            geometry,
            material: self.material(part)?,
            palette_guid: self.palette(part)?,
        })
    }

    fn variant(
        &mut self,
        selection: &PaintSelection,
        tag: Option<SourceUse>,
    ) -> Result<Option<VariantKind>, PaintProvenanceError> {
        let Some(position) = selection.variant.location.clone() else {
            return Ok(None);
        };
        let mut sources = vec![self.forge.at(position, SourceUseRole::Selector)?];
        sources.extend(tag);
        for location in &selection.variant.tag_locations {
            sources.push(self.forge.at(location.clone(), SourceUseRole::Selector)?);
        }
        for input in &selection.compatibility {
            sources.push(match input {
                PaintSelectionInput::DataForge(location) => {
                    self.forge.at(location.clone(), SourceUseRole::Selector)?
                }
                PaintSelectionInput::Xml(entry, location) => archive_scope(self.uses, entry)?
                    .at(location.clone(), SourceUseRole::Selector)?,
            });
        }
        self.record("/variantKind", &sources, false)?;
        Ok(Some(selection.variant.kind))
    }

    fn material(&mut self, part: &Part) -> Result<Option<String>, PaintProvenanceError> {
        let Some((_, original)) = part
            .materials()
            .library()
            .zip(part.material_archive_origins().library())
        else {
            return Ok(None);
        };
        let directory = self.uses.observe(
            SourceInput::Archive(original),
            InputScope::ArchiveDirectoryMetadata,
        )?;
        let (mut sources, _) = directory_path_uses(&directory, original.ancestors().len(), true)?;
        sources.push(archive_scope(self.uses, original)?.at(
            SourceLocation::Opaque {
                reason: SourceCoordinateOmission::Unavailable,
            },
            SourceUseRole::Selector,
        )?);
        if let Some(location) = part.material_selector() {
            sources.push(self.forge.at(location, SourceUseRole::Selector)?);
        }
        self.record("/material", &sources, false)?;
        Ok(Some(original.entry().path().to_owned()))
    }

    fn palette(&mut self, part: &Part) -> Result<Option<String>, PaintProvenanceError> {
        let Some(source) = part.palette_source() else {
            return Ok(None);
        };
        let Some((record, scope)) = source
            .records()
            .palette
            .zip(forge_scope(self.uses, source.forge_origin())?)
        else {
            return Ok(None);
        };
        let mut sources = vec![scope.at(header(record.index, "guid"), SourceUseRole::Payload)?];
        let selection = palette_selector(self.uses, source)?;
        let complete = selection.is_some()
            && (!matches!(source.selection(), PaletteSelection::MaterialDefault { .. })
                || source.records().selector.is_some());
        sources.extend(selection);
        if let Some(lookup) = source.records().selector {
            sources.push(scope.at(header(lookup.index, "fileName"), SourceUseRole::Selector)?);
        }
        self.record("/paletteGuid", &sources, complete)?;
        Ok(Some(record.guid.to_string()))
    }

    fn single(
        &mut self,
        path: &str,
        location: SourceLocation,
        role: SourceUseRole,
    ) -> Result<(), PaintProvenanceError> {
        let used = self.forge.at(location, role)?;
        self.record(path, &[used], true)
    }

    fn record(
        &mut self,
        path: &str,
        uses: &[SourceUse],
        complete: bool,
    ) -> Result<(), PaintProvenanceError> {
        self.fields
            .insert(FieldPath::parse(path)?, self.uses.origin(uses, complete))?;
        Ok(())
    }
}

fn header(record: u32, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: "dataforgeRecordHeader".into(),
        indices: vec![u64::from(record)],
        field: field.into(),
    }
}

fn palette_selector(
    uses: &SourceUses,
    palette: &PaletteSource,
) -> Result<Option<SourceUse>, SourceEvidenceError> {
    let selected = match palette.selection() {
        PaletteSelection::Named { location } => {
            forge_scope(uses, palette.named_origin())?.zip(location.clone())
        }
        PaletteSelection::MaterialDefault { library } => library
            .as_deref()
            .map(|entry| archive_scope(uses, entry))
            .transpose()?
            .zip(palette.material_default_location().cloned()),
        _ => None,
    };
    selected
        .map(|(scope, location)| scope.at(location, SourceUseRole::Selector))
        .transpose()
}

fn forge_scope(
    uses: &SourceUses,
    origin: Option<&DataForgeOrigin>,
) -> Result<Option<SourceScope>, SourceEvidenceError> {
    match origin {
        Some(DataForgeOrigin::Archive(entry)) => archive_scope(uses, entry).map(Some),
        Some(DataForgeOrigin::File(file)) => uses
            .observe(SourceInput::File(file), InputScope::Payload)
            .map(Some),
        _ => Ok(None),
    }
}

fn archive_scope(
    uses: &SourceUses,
    entry: &EntryOrigin,
) -> Result<SourceScope, SourceEvidenceError> {
    uses.observe(SourceInput::Archive(entry), InputScope::Payload)
}
