// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::BTreeMap;

use p4kx_model::{MaterialLibrary, TintEntry, TintPalette};

use super::{Material, MaterialPlan, Slot};

const CAP: &str = r#"<Material><SubMaterials>
<Material Name="hair_scalp_m" Shader="HairPBR" StringGenMask="%HAIR_CAP" Diffuse="0,0,0">
<Textures><Texture Map="TexSlot1" File="Objects/t/sc/female/textures/f_hair_scalp.tif"/></Textures>
<MatLayers><Layer Name="Primary" PaletteTint="1"/></MatLayers>
</Material></SubMaterials></Material>"#;

#[test]
fn a_palette_tint_keeps_the_scalp_map_of_the_body() -> Result<(), Box<dyn std::error::Error>> {
    let library = MaterialLibrary::parse("objects/t/sc/male/m_hair", CAP.as_bytes())?;
    let entry = TintEntry {
        tint: [0.5; 3],
        specular: [0.05; 3],
        glossiness: 100.0,
    };
    let palette = TintPalette::new("p", [entry; 3], [1.0; 3]);
    let mut plan = MaterialPlan {
        library: Some(&library),
        slots: BTreeMap::from([(0, Some(Slot::Library(0)))]),
        default: Cow::Owned(Material::engine_default()),
        overrides: BTreeMap::new(),
        factory_bow: false,
    };
    let corrected = library
        .materials
        .first()
        .and_then(|material| material.hair_cap_of_body(&library.path))
        .ok_or("a corrected cap")?;
    plan.overrides.insert(0, corrected);

    let plan = plan.tinted_by(Some(&palette));

    let cap = plan.material(Slot::Library(0));
    assert!(cap.tinted, "the palette applied");
    assert!(
        cap.textures.iter().all(|slot| slot.file.contains("/male/")),
        "{:?}",
        cap.textures
    );
    Ok(())
}
