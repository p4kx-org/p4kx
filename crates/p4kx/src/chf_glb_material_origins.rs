// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Material and image contributions of the emitted CHF GLB.

use std::collections::BTreeMap;

use anyhow::Result;
use p4kx_assembly::{Armour, ArmourPart, PaletteRecords};
use p4kx_extraction::{
    ArchiveDefaultMaterial, DataForgeOrigin, InputScope, MaterialArchiveOrigins, RiggedGlbEmission,
    RiggedJsonAddress, RiggedPartEmission, SourceInput, SourceLocation, SourceUseRole, SourceUses,
    TextureMaterialSource,
};

use super::Contributors;
use super::character::CharacterInput;

pub(super) fn material_indices(
    uses: &SourceUses,
    armour: &Armour,
    emission: &RiggedGlbEmission,
    forge: &DataForgeOrigin,
    input: &CharacterInput,
) -> Result<BTreeMap<usize, Contributors>> {
    let mut found = BTreeMap::new();
    for &address in emission.json_addresses() {
        let RiggedJsonAddress::Material {
            index,
            part,
            slot,
            archive_default,
        } = address
        else {
            continue;
        };
        let Some(emitted) = emission.material(index) else {
            continue;
        };
        let mut origin = Contributors::default();
        if let Some(slot) = slot {
            if let Some(part) = armour.parts().get(part) {
                origin = library_material(
                    uses,
                    part,
                    forge,
                    input,
                    (slot, emitted.uses_diffuse()),
                    emitted.layers(),
                )?;
            } else {
                origin.incomplete();
            }
        } else if archive_default {
            let layers: Vec<_> = emitted.layers().collect();
            default_material(
                uses,
                &mut origin,
                armour.resolved_default_material(),
                &layers,
            )?;
        }
        found.insert(index, origin);
    }
    Ok(found)
}

pub(super) fn part_materials(
    uses: &SourceUses,
    part: &ArmourPart,
    emitted: &RiggedPartEmission,
    forge: &DataForgeOrigin,
    input: &CharacterInput,
) -> Result<Contributors> {
    let mut found = Contributors::default();
    for slot in emitted.material_slots() {
        let origin = library_material(
            uses,
            part,
            forge,
            input,
            (slot, emitted.material_uses_diffuse(slot)),
            emitted.material_layers(slot),
        )?;
        found.extend(&origin);
    }
    Ok(found)
}

fn library_material(
    uses: &SourceUses,
    part: &ArmourPart,
    forge: &DataForgeOrigin,
    input: &CharacterInput,
    (slot, uses_diffuse): (usize, bool),
    layers: impl Iterator<Item = usize>,
) -> Result<Contributors> {
    let mut source = Contributors::default();
    let archive = part.material_archive_origins();
    source.entry(uses, archive.library(), SourceUseRole::Conversion)?;
    dataforge_selector(uses, &mut source, forge, part.material_selector())?;
    if uses_diffuse {
        input.look(&mut source, part.look_sources_for(slot))?;
    }
    for layer in layers {
        selected_layer(uses, &mut source, archive, slot, layer)?;
    }
    if let Some(records) = part.palette_records_for(slot) {
        palette(uses, &mut source, forge, records)?;
    }
    Ok(source)
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
        default_material(
            uses,
            found,
            armour.resolved_default_material(),
            &source.layers,
        )?;
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

pub(super) fn default_material<'a>(
    uses: &SourceUses,
    found: &mut Contributors,
    fallback: Option<&ArchiveDefaultMaterial>,
    layers: impl IntoIterator<Item = &'a usize>,
) -> Result<()> {
    let Some(fallback) = fallback else {
        found.incomplete();
        return Ok(());
    };
    let archive = fallback.material_origins();
    found.entry(uses, fallback.original(), SourceUseRole::Conversion)?;
    for &layer in layers {
        selected_layer(uses, found, archive, 0, layer)?;
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

pub(super) fn dataforge_selector(
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
