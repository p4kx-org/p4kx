// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use anyhow::Result;
use p4kx_assembly::{Armour, ArmourPart, RigArchiveRead};
use p4kx_extraction::model::ModelFile;
use p4kx_extraction::{
    DataForgeOrigin, InputScope, ModelArchiveSource, SourceInput, SourceLocation, SourceUseRole,
    SourceUses,
};
use p4kx_extraction::{RiggedGlbEmission, RiggedPartEmission};

use super::Contributors;
use super::character::CharacterInput;
use super::material::{dataforge_selector, default_material, material_indices, part_materials};

#[derive(Default)]
pub(super) struct Parts {
    pub(super) shape: Contributors,
    pub(super) nodes: Contributors,
    pub(super) bones: Contributors,
    pub(super) geometry: Contributors,
    pub(super) morphs: Contributors,
    pub(super) materials: Contributors,
    pub(super) individual: Vec<Part>,
    pub(super) material_indices: BTreeMap<usize, Contributors>,
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
            let mut origins = Part::read(uses, part, emitted, forge, &found.shape, input)?;
            if let Some(layers) = emitted.archive_default_layers() {
                let mut fallback = Contributors::default();
                default_material(
                    uses,
                    &mut fallback,
                    armour.resolved_default_material(),
                    layers,
                )?;
                origins.materials.extend(&fallback);
            }
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
        found.material_indices = material_indices(uses, armour, emission, forge, input)?;
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
        found.materials = part_materials(uses, part, emitted, forge, input)?;
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
