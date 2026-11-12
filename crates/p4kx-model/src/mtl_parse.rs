// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::mtl::MAX_SUB_MATERIALS;
use crate::mtl_material_read;
use crate::mtl_raw::Budget;
use crate::mtl_read_site::{FieldElement, FieldToken, MaterialElement, ReadSite};
use crate::mtl_source::{MaterialEntryDisposition, MaterialSource, MaterialSourceEntry};
use crate::mtl_source_error::MaterialReadError;
use crate::mtl_xml::XmlElement;
use crate::{
    Material, MaterialError, MaterialLibrary, MaterialReadField, MaterialReadObservation,
    MaterialReadUse,
};

pub(crate) fn library<'a, Interrupted>(
    path: &str,
    root: impl XmlElement<'a>,
    mut authored: MaterialSource,
    checkpoint: &mut impl FnMut() -> Result<(), Interrupted>,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<MaterialLibrary, MaterialReadError<Interrupted>> {
    if !root.name().eq_ignore_ascii_case("Material") {
        return Err(MaterialError::NotMaterial {
            found: root.name().to_owned(),
        }
        .into());
    }
    let default_palette =
        default_palette(root, observer).map_err(MaterialReadError::Interrupted)?;
    let sub_materials = root
        .children()
        .enumerate()
        .find(|(_, child)| child.name().eq_ignore_ascii_case("SubMaterials"));
    observe_selection(sub_materials.map(|(ordinal, _)| ordinal), observer)
        .map_err(MaterialReadError::Interrupted)?;
    let Some(sub_materials) = sub_materials else {
        return single(path, root, authored, default_palette, checkpoint, observer);
    };
    let (materials, unknown_entries) = submaterials(
        sub_materials.0,
        sub_materials.1,
        &mut authored,
        checkpoint,
        observer,
    )?;
    Ok(MaterialLibrary {
        path: path.to_owned(),
        target: None,
        authored: Arc::new(authored),
        default_palette,
        single: false,
        unknown_entries,
        unresolved_layers: Vec::new(),
        materials,
    })
}

fn default_palette<'source, Interrupted>(
    root: impl XmlElement<'source>,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<Option<String>, Interrupted> {
    let palette = root.positioned_attribute("DefaultPalette");
    if let Some(palette) = palette.filter(|palette| !palette.value.is_empty()) {
        ReadSite::ROOT.token(
            FieldElement::Material,
            MaterialReadField::LibraryDefaultPalette,
            FieldToken {
                name: palette.name,
                ordinal: Some(palette.ordinal),
                use_kind: MaterialReadUse::Authored,
            },
            observer,
        )?;
        Ok(Some(palette.value.to_owned()))
    } else {
        Ok(None)
    }
}

fn observe_selection<Interrupted>(
    list: Option<usize>,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<(), Interrupted> {
    let selection = list.map_or(FieldElement::Material, |ordinal| FieldElement::Child {
        ordinal,
    });
    for (field, label) in [
        (MaterialReadField::LibrarySingle, "SubMaterials"),
        (MaterialReadField::LibraryTotal, "materialSlots"),
        (MaterialReadField::LibraryUnknownCount, "materialSlotKinds"),
    ] {
        ReadSite::ROOT.at(selection, field, label, MaterialReadUse::Selected, observer)?;
    }
    Ok(())
}

fn single<'source, Interrupted>(
    path: &str,
    root: impl XmlElement<'source>,
    authored: MaterialSource,
    default_palette: Option<String>,
    checkpoint: &mut impl FnMut() -> Result<(), Interrupted>,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<MaterialLibrary, MaterialReadError<Interrupted>> {
    checkpoint().map_err(MaterialReadError::Interrupted)?;
    let site = ReadSite::ROOT;
    site.disposition(root.name(), observer)
        .map_err(MaterialReadError::Interrupted)?;
    Ok(MaterialLibrary {
        path: path.to_owned(),
        target: None,
        authored: Arc::new(authored),
        default_palette,
        single: true,
        unknown_entries: 0,
        unresolved_layers: Vec::new(),
        materials: vec![mtl_material_read::material(
            root,
            &mut Budget::new(),
            site,
            observer,
        )?],
    })
}

fn submaterials<'a, Interrupted>(
    list: usize,
    sub_materials: impl XmlElement<'a>,
    authored: &mut MaterialSource,
    checkpoint: &mut impl FnMut() -> Result<(), Interrupted>,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<(Vec<Material>, usize), MaterialReadError<Interrupted>> {
    let mut materials = Vec::new();
    let mut unknown_entries = 0;
    for (ordinal, child) in sub_materials.children().enumerate() {
        checkpoint().map_err(MaterialReadError::Interrupted)?;
        if materials.len() == MAX_SUB_MATERIALS {
            return Err(MaterialError::TooMany {
                what: "sub-materials",
                limit: MAX_SUB_MATERIALS,
            }
            .into());
        }
        authored.entries.push(MaterialSourceEntry {
            ordinal: materials.len(),
            element_name: child.name().to_owned(),
            disposition: if child.name().eq_ignore_ascii_case("Material") {
                MaterialEntryDisposition::Material
            } else {
                MaterialEntryDisposition::EngineDefault
            },
        });
        let site = ReadSite {
            material: materials.len(),
            element: MaterialElement::Submaterial { list, ordinal },
        };
        site.disposition(child.name(), observer)
            .map_err(MaterialReadError::Interrupted)?;
        if child.name().eq_ignore_ascii_case("Material") {
            materials.push(mtl_material_read::material(
                child,
                &mut Budget::new(),
                site,
                observer,
            )?);
        } else {
            unknown_entries += 1;
            for field in [
                MaterialReadField::Textures,
                MaterialReadField::PublicParams,
                MaterialReadField::Layers,
            ] {
                site.at(
                    FieldElement::Material,
                    field,
                    "engineDefaultSlot",
                    MaterialReadUse::Selected,
                    observer,
                )
                .map_err(MaterialReadError::Interrupted)?;
            }
            materials.push(Material::engine_default());
        }
    }
    Ok((materials, unknown_entries))
}
