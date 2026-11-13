// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use super::{EmissionAttributeKind, EmissionNumber, EmissionNumeric, MaterialEmission};
use crate::MaterialLibrary;

#[test]
fn original_emission_controls_keep_absence_zero_nonfinite_tokens_and_physical_slots()
-> Result<(), Box<dyn Error>> {
    let bytes = br#"<Material xmlns:vendor="urn:opaque">
        <Unexpected/>
        <SubMaterials>
            <UnknownMaterial/>
            <Material Shader="HardSurface" MtlFlags="0x8000" Glow="0" Emissive="0.25,0.5,1"
                Opacity="0.75" AlphaTest="NaN" vendor:Glow="9">
                <PublicParams AlphaGlowCutoff="INF" EmissiveTint="0,0,0" ExtraUnknown="Data"/>
                <Textures>
                    <Texture Map="Diffuse" File="glow_name_only.dds"/>
                    <Texture Map="TexSlot14" File="explicit_role.dds"/>
                    <Texture Map="Emissive"/>
                </Textures>
            </Material>
            <Material Emissive="1, 1, 1" Glow="invalid"/>
            <Material Glow="2" StringGenMask="%NORMAL_MAP%MACRO_EMISSIVE_MAP"/>
        </SubMaterials>
    </Material>"#;
    let library = MaterialLibrary::parse("synthetic.mtl", bytes)?;
    assert!(
        library.emission_source(0).is_none(),
        "unknown physical slot is not reinterpreted"
    );
    let source = library.emission_source(1).ok_or("actual authored slot")?;
    verify_source(&source)?;
    let invalid = library.emission_source(2).ok_or("invalid Glow retained")?;
    assert_eq!(
        number(&invalid, EmissionAttributeKind::Glow),
        Some(EmissionNumeric::Invalid),
        "invalid remains visible"
    );
    assert_eq!(invalid.forward_enabled, None, "absent bitset stays absent");
    let mapped = library.emission_source(3).ok_or("generation mask")?;
    assert_eq!(
        mapped.emittance_requested,
        Some(true),
        "explicit shader declaration"
    );
    assert_eq!(
        number(&mapped, EmissionAttributeKind::Glow),
        Some(EmissionNumeric::Scalar(EmissionNumber::Finite(2.0))),
        "positive authored input"
    );
    Ok(())
}

fn verify_source(source: &MaterialEmission<'_>) -> Result<(), Box<dyn Error>> {
    assert_eq!(
        source.element.as_slice(),
        [1, 1],
        "original skipped child and slot positions"
    );
    assert!(source.source_complete, "authored XML retained completely");
    assert_eq!(
        source.forward_enabled,
        Some(true),
        "original hex flag bitset"
    );
    assert_eq!(source.emittance_requested, None, "no permutation assumed");
    assert_eq!(
        number(source, EmissionAttributeKind::Glow),
        Some(EmissionNumeric::Scalar(EmissionNumber::Finite(0.0))),
        "authored zero is distinct from absence",
    );
    assert_eq!(
        number(source, EmissionAttributeKind::AlphaTest),
        Some(EmissionNumeric::Scalar(EmissionNumber::Nan)),
        "nonfinite alpha cutoff is retained",
    );
    assert_eq!(
        number(source, EmissionAttributeKind::AlphaGlowCutoff),
        Some(EmissionNumeric::Scalar(EmissionNumber::PositiveInfinity)),
        "nonfinite shader parameter is retained",
    );
    let [slot, missing_file] = source.slots.as_slice() else {
        return Err("two explicit source slots".into());
    };
    assert_eq!(slot.map.raw.value, "TexSlot14", "exact authored slot");
    assert_eq!(
        slot.map.element.as_slice(),
        [1, 1, 1, 1],
        "original slot path"
    );
    assert_eq!(
        slot.file
            .as_ref()
            .and_then(|file| file.raw.lexeme.as_deref()),
        Some("explicit_role.dds"),
        "original source token",
    );
    assert!(
        missing_file.file.is_none(),
        "explicit slot without inferred filename"
    );
    Ok(())
}

fn number(source: &MaterialEmission<'_>, kind: EmissionAttributeKind) -> Option<EmissionNumeric> {
    source
        .attributes
        .iter()
        .find(|field| field.kind == kind)
        .map(|field| field.value.interpreted)
}

#[test]
fn cryxml_preserves_source_token_without_a_text_lexeme() -> Result<(), Box<dyn Error>> {
    let material = p4kx_testkit::TestMtlMaterial::new("authored")
        .attribute("MtlFlags", "32768")
        .attribute("Emissive", "0,0,0");
    let bytes = p4kx_testkit::TestMtl::single(&material).cryxml();
    let library = MaterialLibrary::parse("synthetic.mtl", &bytes)?;
    let source = library
        .emission_source(0)
        .ok_or("single authored material")?;
    assert_eq!(
        source.forward_enabled,
        Some(true),
        "decimal authored flag bits"
    );
    assert_eq!(
        number(&source, EmissionAttributeKind::Emissive),
        Some(EmissionNumeric::Colour([EmissionNumber::Finite(0.0); 3])),
        "black stays black at the source boundary",
    );
    assert_eq!(
        number(&source, EmissionAttributeKind::Glow),
        None,
        "missing source scalar remains absent",
    );
    let field = source
        .attributes
        .iter()
        .find(|field| field.kind == EmissionAttributeKind::Flags)
        .ok_or("original flag")?;
    assert_eq!(
        field.value.raw.lexeme, None,
        "binary data carries no XML token span"
    );
    assert_eq!(
        field.value.raw.value, "32768",
        "independently retained original value"
    );
    Ok(())
}
