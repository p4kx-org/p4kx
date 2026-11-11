// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use anyhow::Result;
use p4kx_assembly::{Armour, ArmourPart, PaletteRecords, RigArchiveRead};
use p4kx_extraction::model::ModelFile;
use p4kx_extraction::{
    DataForgeOrigin, InputScope, MaterialArchiveOrigins, ModelArchiveSource, SourceInput,
    SourceLocation, SourceUseRole, SourceUses, TextureMaterialSource,
};
use p4kx_extraction::{RiggedGlbEmission, RiggedPartEmission};

use super::Contributors;
use super::character::CharacterInput;

#[derive(Default)]
pub(super) struct Parts {
    pub(super) shape: Contributors,
    pub(super) nodes: Contributors,
    pub(super) bones: Contributors,
    pub(super) geometry: Contributors,
    pub(super) morphs: Contributors,
    pub(super) materials: Contributors,
    pub(super) individual: Vec<Part>,
}

#[derive(Default)]
pub(super) struct Part {
    pub(super) selection: Contributors,
    pub(super) path: Contributors,
    face: Contributors,
    pub(super) geometry: Contributors,
    pub(super) shaping: Contributors,
    pub(super) shaped: bool,
    pub(super) morphs: Contributors,
    pub(super) materials: Contributors,
    pub(super) material_slots: BTreeMap<usize, Contributors>,
}

impl Parts {
    pub(super) fn read(
        uses: &SourceUses,
        armour: &Armour,
        emission: &RiggedGlbEmission,
        bones_written: bool,
        forge: &DataForgeOrigin,
        shape: Contributors,
        input: &CharacterInput,
    ) -> Result<Self> {
        let mut found = Self {
            shape,
            ..Self::default()
        };
        if bones_written {
            add_model(uses, &mut found.bones, armour.skeleton_archive_source())?;
        }
        for (part, emitted) in armour.parts().iter().zip(emission.parts()) {
            let origins = Part::read(uses, part, emitted, forge, &found.shape, input)?;
            if !part.model().nodes.is_empty() {
                found.nodes.extend(&origins.selection);
            }
            if bones_written && part.has_facial_rig() {
                found.bones.extend(&origins.face);
            }
            found.geometry.extend(&origins.geometry);
            found.geometry.extend(&origins.shaping);
            found.morphs.extend(&origins.morphs);
            found.materials.extend(&origins.materials);
            found.individual.push(origins);
        }
        Ok(found)
    }
}

impl Part {
    fn read(
        uses: &SourceUses,
        part: &ArmourPart,
        emitted: &RiggedPartEmission,
        forge: &DataForgeOrigin,
        shape_choice: &Contributors,
        input: &CharacterInput,
    ) -> Result<Self> {
        let mut found = Self {
            shaped: !part.shape_archive_sources().is_empty(),
            ..Self::default()
        };
        let files = part.model_archive_sources();
        (found.selection, found.path) = selected_part(uses, part, forge)?;
        if emitted.geometry() {
            found.geometry.extend(&found.selection);
            for source in files
                .iter()
                .filter(|source| source.file() != ModelFile::Model)
            {
                if !matches!(source.file(), ModelFile::LodModel(_)) || emitted.skinned() {
                    add_model(uses, &mut found.geometry, Some(source))?;
                }
            }
            for shape in part.shape_archive_sources() {
                rig(uses, &mut found.shaping, &shape.rig)?;
                for model in shape.model.iter() {
                    add_model(uses, &mut found.shaping, Some(model))?;
                }
            }
        }
        if part.has_facial_rig() {
            found.face.extend(&found.selection);
            if found.shaped {
                found.face.extend(shape_choice);
            }
            for shape in part.shape_archive_sources() {
                rig(uses, &mut found.face, &shape.rig)?;
            }
        }
        if emitted.morphs() {
            found.morphs.extend(&found.geometry);
            found.morphs.extend(&found.shaping);
            if found.shaped {
                found.morphs.extend(shape_choice);
            }
            for source in part
                .shape_archive_sources()
                .iter()
                .filter_map(|shape| shape.lower_morphs.as_ref())
            {
                rig(uses, &mut found.morphs, &source.rig)?;
                for model in source.model.iter() {
                    add_model(uses, &mut found.morphs, Some(model))?;
                }
            }
        }
        found.material_slots = materials(uses, part, emitted, forge, input)?;
        for material in found.material_slots.values() {
            found.materials.extend(material);
        }
        Ok(found)
    }
}

fn selected_part(
    uses: &SourceUses,
    part: &ArmourPart,
    forge: &DataForgeOrigin,
) -> Result<(Contributors, Contributors)> {
    let mut model = Contributors::default();
    let selector = part.geometry_selector();
    dataforge_selector(uses, &mut model, forge, selector)?;
    let primary = part
        .model_archive_sources()
        .iter()
        .find(|source| source.file() == ModelFile::Model);
    add_model(uses, &mut model, primary)?;
    if let Some(definition) = part.model_definition_origin() {
        model.entry(uses, Some(definition), SourceUseRole::Selector)?;
    }
    let named = part
        .model_definition_origin()
        .or_else(|| primary.and_then(ModelArchiveSource::origin));
    let scope = uses.observe(
        named.map_or(SourceInput::Unknown, SourceInput::Archive),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    let mut path = Contributors {
        values: vec![scope.at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            SourceUseRole::Selector,
        )?],
        complete: named.is_some(),
    };
    dataforge_selector(uses, &mut path, forge, selector)?;
    Ok((model, path))
}

fn add_model(
    uses: &SourceUses,
    found: &mut Contributors,
    source: Option<&ModelArchiveSource>,
) -> Result<()> {
    found.entry(
        uses,
        source.and_then(ModelArchiveSource::origin),
        SourceUseRole::Conversion,
    )
}

pub(super) fn rig(uses: &SourceUses, found: &mut Contributors, rig: &RigArchiveRead) -> Result<()> {
    found.entry(uses, rig.entry.as_ref(), SourceUseRole::Conversion)
}

fn materials(
    uses: &SourceUses,
    part: &ArmourPart,
    emitted: &RiggedPartEmission,
    forge: &DataForgeOrigin,
    input: &CharacterInput,
) -> Result<BTreeMap<usize, Contributors>> {
    let mut found = BTreeMap::new();
    let archive = part.material_archive_origins();

    for slot in emitted.material_slots() {
        let mut source = Contributors::default();
        source.entry(uses, archive.library(), SourceUseRole::Conversion)?;
        dataforge_selector(uses, &mut source, forge, part.material_selector())?;
        if emitted.material_uses_diffuse(slot) {
            input.look(&mut source, part.look_sources_for(slot))?;
        }
        for layer in emitted.material_layers(slot) {
            selected_layer(uses, &mut source, archive, slot, layer)?;
        }
        if let Some(records) = part.palette_records_for(slot) {
            palette(uses, &mut source, forge, records)?;
        }
        found.insert(slot, source);
    }
    Ok(found)
}

pub(super) fn image_material(
    uses: &SourceUses,
    found: &mut Contributors,
    armour: &Armour,
    forge: &DataForgeOrigin,
    source: Option<&TextureMaterialSource>,
    input: &CharacterInput,
) -> Result<()> {
    let Some(source) = source else {
        found.incomplete();
        return Ok(());
    };
    let Some(slot) = source.slot else {
        return Ok(());
    };
    let Some(part) = armour.parts().get(source.part) else {
        found.incomplete();
        return Ok(());
    };
    let archive = part.material_archive_origins();
    found.entry(uses, archive.library(), SourceUseRole::Selector)?;
    dataforge_selector(uses, found, forge, part.material_selector())?;
    if source.uses_diffuse {
        input.look(found, part.look_sources_for(slot))?;
    }
    for &layer in &source.layers {
        selected_layer(uses, found, archive, slot, layer)?;
    }
    if source.uses_palette {
        if let Some(records) = part.palette_records_for(slot) {
            palette(uses, found, forge, records)?;
        } else {
            found.incomplete();
        }
    }
    Ok(())
}

fn selected_layer(
    uses: &SourceUses,
    found: &mut Contributors,
    archive: &MaterialArchiveOrigins,
    slot: usize,
    layer: usize,
) -> Result<()> {
    if let Some(origin) = archive.layer(slot, layer)
        && let Some(material) = origin.source_material()
    {
        let scope = uses.observe(SourceInput::Archive(origin.entry()), InputScope::Payload)?;
        found.values.push(scope.at(
            SourceLocation::Subresource {
                namespace: "materialLibrary".into(),
                indices: vec![material as u64],
                field: "selectedMaterial".into(),
            },
            SourceUseRole::Conversion,
        )?);
    } else {
        found.incomplete();
    }
    Ok(())
}

fn dataforge_selector(
    uses: &SourceUses,
    found: &mut Contributors,
    forge: &DataForgeOrigin,
    location: Option<&SourceLocation>,
) -> Result<()> {
    if let Some(location) = location {
        let scope = uses.observe(dataforge_input(forge), InputScope::Payload)?;
        found
            .values
            .push(scope.at(location.clone(), SourceUseRole::Selector)?);
    }
    Ok(())
}

const fn dataforge_input(forge: &DataForgeOrigin) -> SourceInput<'_> {
    match forge {
        DataForgeOrigin::Archive(entry) => SourceInput::Archive(entry),
        _ => SourceInput::Unknown,
    }
}

fn palette(
    uses: &SourceUses,
    found: &mut Contributors,
    forge: &DataForgeOrigin,
    records: PaletteRecords,
) -> Result<()> {
    let scope = uses.observe(dataforge_input(forge), InputScope::Payload)?;
    if let Some(selector) = records.selector {
        found.values.push(scope.at(
            SourceLocation::Subresource {
                namespace: "dataForgeRecordHeader".into(),
                indices: vec![u64::from(selector.index)],
                field: "guid".into(),
            },
            SourceUseRole::Selector,
        )?);
    }
    if let Some(record) = records.palette {
        found.values.push(scope.at(
            SourceLocation::DataForge {
                record_index: record.index,
                field: "root".into(),
            },
            SourceUseRole::Conversion,
        )?);
    } else {
        found.incomplete();
    }
    Ok(())
}
