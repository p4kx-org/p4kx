// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_model::MaterialLibrary;

use super::{AuthoredEmission, EmissionSample, PackageMaterialEmission};
use crate::material_emission_origins::EmissionEvidence;

const MATERIAL: &[u8] = br#"<Material><SubMaterials>
    <Material Shader="Illum" MtlFlags="32768" Emissive="0.25,0.5,1">
        <Textures><Texture Map="Diffuse" File="colour.dds"/></Textures>
    </Material>
    <Material Shader="Illum" MtlFlags="32768" Emissive="0.25,0.5,1" Glow="0">
        <PublicParams AlphaGlowCutoff="0"/>
        <Textures><Texture Map="Diffuse" File="colour.dds"/></Textures>
    </Material>
    <Material Shader="Illum" MtlFlags="32768" Emissive="0.25,0.5,1" Glow="2">
        <PublicParams AlphaGlowCutoff="0.5"/>
        <Textures><Texture Map="Diffuse" File="colour.dds"/></Textures>
    </Material>
    <Material Shader="Illum" MtlFlags="32768" Emissive="0.25,0.5,1" Glow="2"
        StringGenMask="%EMITTANCE_MAP">
        <PublicParams AlphaGlowCutoff="0.5"/>
        <Textures>
            <Texture Map="Diffuse" File="colour.dds"/>
            <Texture Map="Emittance" File="emission.dds"/>
        </Textures>
    </Material>
</SubMaterials></Material>"#;

#[test]
fn forward_illum_retains_authored_inputs_and_requires_an_explicit_emittance_route()
-> Result<(), Box<dyn Error>> {
    let library = MaterialLibrary::parse("forward-illum.mtl", MATERIAL)?;
    let evidence = EmissionEvidence::new(None)?;
    let mut rows = Vec::new();
    for (index, material) in library.materials.iter().enumerate() {
        let source = library.emission_source(index).ok_or("authored material")?;
        rows.push(PackageMaterialEmission::of(
            &source, 0, material, &evidence,
        )?);
    }
    let [absent, zero, positive, mapped] = rows.as_slice() else {
        return Err("all four Forward Illum slots must be present".into());
    };
    for row in &rows {
        assert!(row.source_complete, "the original XML is retained");
        assert_eq!(row.forward_enabled, Some(true), "the authored forward flag");
    }
    for row in [absent, zero, positive] {
        assert_eq!(row.emittance_requested, None, "no authored emittance mask");
        assert!(
            row.texture_slots.is_empty(),
            "Diffuse is not an emission slot",
        );
        assert!(
            row.preview.is_none(),
            "no static emittance texture was authored",
        );
    }
    assert!(scalar(absent, "Glow").is_none(), "Glow is absent in source");
    assert!(
        scalar(absent, "AlphaGlowCutoff").is_none(),
        "AlphaGlowCutoff is absent in source"
    );
    assert_eq!(
        scalar(zero, "Glow"),
        Some(EmissionSample::Finite(0.0)),
        "the authored zero Glow",
    );
    assert_eq!(
        scalar(zero, "AlphaGlowCutoff"),
        Some(EmissionSample::Finite(0.0)),
        "the authored zero cutoff",
    );
    assert_eq!(
        scalar(positive, "Glow"),
        Some(EmissionSample::Finite(2.0)),
        "valid Glow is retained even without an emittance route",
    );
    assert_eq!(
        scalar(positive, "AlphaGlowCutoff"),
        Some(EmissionSample::Finite(0.5)),
        "positive cutoff is retained as authored data",
    );
    verify_explicit_emittance(mapped)
}

fn verify_explicit_emittance(mapped: &PackageMaterialEmission) -> Result<(), Box<dyn Error>> {
    assert_eq!(mapped.emittance_requested, Some(true), "authored mask");
    let [slot] = mapped.texture_slots.as_slice() else {
        return Err("only the explicit Emittance slot must be present".into());
    };
    assert_eq!(slot.map.element, [0, 3, 1, 1], "original Emittance element");
    assert_eq!(slot.map.attribute, 0, "original Map attribute");
    assert_eq!(slot.map.raw, "Emittance", "the explicit slot name survives");
    let preview = mapped
        .preview
        .as_ref()
        .ok_or("explicit emittance preview")?;
    assert_eq!(preview.slot, "Emittance", "the preview binds Emittance");
    assert_eq!(
        preview.file, "emission.dds",
        "the original file is selected"
    );
    assert_eq!(
        preview.tint.map(f32::to_bits),
        [0.25, 0.5, 1.0].map(f32::to_bits),
        "linear authored tint",
    );
    assert_eq!(
        preview.strength.to_bits(),
        2.0_f32.to_bits(),
        "authored positive Glow",
    );
    Ok(())
}

fn scalar(row: &PackageMaterialEmission, name: &str) -> Option<EmissionSample> {
    row.attributes.iter().find_map(|field| match field.value {
        AuthoredEmission::Scalar(sample) if field.source.name == name => Some(sample),
        _ => None,
    })
}
