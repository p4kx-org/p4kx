// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForgeOrigin, EntryOrigin, FieldOrigins, FieldPath, InputScope, SourceCoordinateOmission,
    SourceEvidenceError, SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses, directory_path_uses,
};

use super::{
    PaintAppearanceValues, PaintProvenanceError, PaintSelection, PaintSelectionInput, VariantKind,
};
use crate::vehicle_paint::record_header as header;
use crate::{Assembly, PaletteSelection, PaletteSource, Part};

pub(super) fn capture(
    assembly: &Assembly,
    selection: &PaintSelection,
    part: &Part,
    uses: &SourceUses,
) -> Result<Option<(PaintAppearanceValues, FieldOrigins)>, PaintProvenanceError> {
    let Some(forge) = forge_scope(uses, assembly.forge_origin.as_deref())? else {
        return Ok(None);
    };
    let mut capture = Capture {
        uses,
        forge: &forge,
        fields: FieldOrigins::new(),
    };
    let appearance = capture.appearance(selection, part)?;
    Ok(Some((appearance, capture.fields)))
}

struct Capture<'capture> {
    uses: &'capture SourceUses,
    forge: &'capture SourceScope,
    fields: FieldOrigins,
}

struct MaterialChoice<'part> {
    entry: &'part EntryOrigin,
    uses: Vec<SourceUse>,
}

impl Capture<'_> {
    fn appearance(
        &mut self,
        selection: &PaintSelection,
        part: &Part,
    ) -> Result<PaintAppearanceValues, PaintProvenanceError> {
        let mut decision = self.fitted_sources(selection)?;
        let appearance_tag = self.tag(selection, &mut decision)?;
        let paint_guid = selection
            .record
            .map(|record_index| {
                let used = self
                    .forge
                    .at(header(record_index, "guid"), SourceUseRole::Payload)?;
                self.record_with("/paintGuid", &mut decision, [used])?;
                Ok::<_, PaintProvenanceError>(selection.guid.to_string())
            })
            .transpose()?;
        let variant_kind = self.variant(selection, &mut decision)?;
        let geometry = part
            .geometry()
            .zip(part.geometry_selector())
            .map(|(path, location)| {
                let used = self.forge.at(location, SourceUseRole::Selector)?;
                self.record_with("/geometry", &mut decision, [used])?;
                Ok::<_, PaintProvenanceError>(path.to_owned())
            })
            .transpose()?;
        let material = self.material(part)?;
        if let Some(choice) = &material {
            self.record_with("/material", &mut decision, choice.uses.iter().cloned())?;
        }
        let palette_guid = self.palette(part, &mut decision, material.as_ref())?;
        Ok(PaintAppearanceValues {
            paint_guid,
            appearance_tag,
            variant_kind,
            geometry,
            material: material.map(|choice| choice.entry.entry().path().to_owned()),
            palette_guid,
        })
    }

    fn fitted_sources(
        &self,
        selection: &PaintSelection,
    ) -> Result<Vec<SourceUse>, SourceEvidenceError> {
        selection
            .compatibility
            .iter()
            .map(|input| match input {
                PaintSelectionInput::DataForge(location) => {
                    self.forge.at(location.clone(), SourceUseRole::Selector)
                }
                PaintSelectionInput::Xml(entry, location) => {
                    archive_scope(self.uses, entry)?.at(location.clone(), SourceUseRole::Selector)
                }
            })
            .collect()
    }

    fn tag(
        &mut self,
        selection: &PaintSelection,
        decision: &mut Vec<SourceUse>,
    ) -> Result<Option<String>, PaintProvenanceError> {
        let Some((record_index, component)) = selection.record.zip(selection.attachable) else {
            return Ok(None);
        };
        let used = self.forge.at(
            SourceLocation::DataForge {
                record_index,
                field: format!("Components[{component}].AttachDef.Tags"),
            },
            SourceUseRole::Selector,
        )?;
        decision.push(used);
        self.record("/appearanceTag", decision)?;
        Ok(Some(selection.tag.clone()))
    }

    fn variant(
        &mut self,
        selection: &PaintSelection,
        decision: &mut Vec<SourceUse>,
    ) -> Result<Option<VariantKind>, PaintProvenanceError> {
        let Some(position) = selection.variant.location.clone() else {
            return Ok(None);
        };
        decision.push(self.forge.at(position, SourceUseRole::Selector)?);
        for location in &selection.variant.tag_locations {
            decision.push(self.forge.at(location.clone(), SourceUseRole::Selector)?);
        }
        self.record("/variantKind", decision)?;
        Ok(Some(selection.variant.kind))
    }

    fn material<'part>(
        &self,
        part: &'part Part,
    ) -> Result<Option<MaterialChoice<'part>>, SourceEvidenceError> {
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
        let authored = if let Some((definition, selection)) = part.cdf_material_sources() {
            let scope = archive_scope(self.uses, definition)?;
            for location in selection.model().chain(selection.material()) {
                sources.push(scope.at(location, SourceUseRole::Selector)?);
            }
            part.geometry_selector()
        } else if let Some((model, location)) = part.embedded_material_selector() {
            sources.push(archive_scope(self.uses, model)?.at(location, SourceUseRole::Selector)?);
            part.geometry_selector()
        } else {
            part.material_selector()
        };
        if let Some(location) = authored {
            sources.push(self.forge.at(location, SourceUseRole::Selector)?);
        }
        Ok(Some(MaterialChoice {
            entry: original,
            uses: sources,
        }))
    }

    fn palette(
        &mut self,
        part: &Part,
        decision: &mut Vec<SourceUse>,
        material: Option<&MaterialChoice<'_>>,
    ) -> Result<Option<String>, PaintProvenanceError> {
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
        sources.extend(palette_selector(self.uses, source)?);
        if let (
            PaletteSelection::MaterialDefault {
                library: Some(entry),
            },
            Some(material),
        ) = (source.selection(), material)
            && material.entry.binding().is_same_entry(&entry.binding())
        {
            sources.extend(
                material
                    .uses
                    .iter()
                    .filter(|usage| usage.role() == SourceUseRole::Selector)
                    .cloned(),
            );
        }
        if let Some(lookup) = source.records().selector {
            sources.push(scope.at(header(lookup.index, "fileName"), SourceUseRole::Selector)?);
        }
        self.record_with("/paletteGuid", decision, sources)?;
        Ok(Some(record.guid.to_string()))
    }

    fn record_with(
        &mut self,
        path: &str,
        decision: &mut Vec<SourceUse>,
        local_sources: impl IntoIterator<Item = SourceUse>,
    ) -> Result<(), PaintProvenanceError> {
        let common_len = decision.len();
        decision.extend(local_sources);
        let result = self.record(path, decision);
        decision.truncate(common_len);
        result
    }

    fn record(&mut self, path: &str, uses: &[SourceUse]) -> Result<(), PaintProvenanceError> {
        self.fields
            .insert(FieldPath::parse(path)?, self.uses.origin(uses, false))?;
        Ok(())
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
