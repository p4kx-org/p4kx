// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::convert::Infallible;
use std::error::Error;

use p4kx_model::{
    MaterialError, MaterialLibrary, MaterialReadError, MaterialSourceLimits, PaletteSlot,
};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) fn samples() -> Vec<Vec<u8>> {
    let source = TestElement::new("Material")
        .attribute("DefaultPalette", "synthetic/default")
        .child(TestElement::new("Ignored"))
        .child(
            TestElement::new("SubMaterials")
                .child(TestElement::new("MatRef"))
                .child(
                    TestElement::new("Material")
                        .attribute("Name", "typed")
                        .attribute("MtlFlags", "-1")
                        .attribute("Opacity", "0")
                        .attribute("Diffuse", "0.1,0.2,0.3")
                        .attribute("Shininess", "NaN")
                        .child(
                            TestElement::new("MatLayers")
                                .child(
                                    TestElement::new("Layer").attribute("PaletteTint", "invalid"),
                                )
                                .child(TestElement::new("Layer").attribute("PaletteTint", "2"))
                                .child(TestElement::new("Layer").attribute("PaletteTint", "3")),
                        ),
                ),
        );
    vec![
        source.to_xml().into_bytes(),
        source.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
    ]
}

#[test]
fn material_observer_baseline_keeps_unknown_slots_and_first_valid_palette() -> TestResult {
    for bytes in samples() {
        let library = MaterialLibrary::parse("synthetic/observer", &bytes)?;
        assert!(
            !library.single,
            "the explicit list keeps its material-ID scope"
        );
        assert_eq!(
            library.unknown_entries, 1,
            "unknown entries retain their original position"
        );
        assert_eq!(library.materials.len(), 2, "the unknown slot is retained");
        let default = library.materials.first().ok_or("engine-default slot")?;
        assert_eq!(
            default.diffuse, [0.8; 3],
            "unknown slots use engine diffuse"
        );
        let typed = library.materials.get(1).ok_or("typed slot")?;
        assert_eq!(typed.name, "typed", "the copied name survives");
        assert_eq!(
            typed.flags.0,
            u32::MAX,
            "signed source flags preserve their low bits"
        );
        assert_eq!(typed.opacity, 0.0, "explicit zero remains authored");
        assert_eq!(
            typed.diffuse,
            [0.1, 0.2, 0.3],
            "the source colour remains exact"
        );
        assert_eq!(
            typed.shininess, 10.0,
            "non-finite numbers use the declared default"
        );
        assert_eq!(
            typed.palette,
            Some(PaletteSlot::Secondary),
            "the first valid layer wins"
        );
    }
    Ok(())
}

#[test]
fn material_observer_baseline_keeps_numeric_fallbacks_and_explicit_defaults() -> TestResult {
    for (attributes, expected_opacity) in [
        ("", 1.0),
        ("Opacity='bad'", 1.0),
        ("Opacity='1'", 1.0),
        ("Opacity='0'", 0.0),
    ] {
        let xml =
            format!("<Material {attributes} Diffuse='bad' Specular='NaN,0,0' Emissive='0,0,0,0'/>");
        let library = MaterialLibrary::parse("synthetic/defaults", xml.as_bytes())?;
        let material = library.materials.first().ok_or("single material")?;
        assert_eq!(
            material.opacity, expected_opacity,
            "source absence and explicit values preserve existing results"
        );
        assert_eq!(
            material.diffuse, [1.0; 3],
            "invalid diffuse defaults to white"
        );
        assert_eq!(
            material.specular, [0.04; 3],
            "non-finite specular defaults to dielectric"
        );
        assert_eq!(
            material.emissive, [0.0; 3],
            "extra vector components remain invalid"
        );
    }
    Ok(())
}

#[test]
fn material_observer_baseline_keeps_configuration_and_checkpoint_failure_priority() {
    let mut limits = MaterialSourceLimits::default();
    limits.elements = 0;
    let calls = Cell::new(0);
    let result = MaterialLibrary::parse_with("synthetic/limits", b"not XML", limits, || {
        calls.set(calls.get() + 1);
        Err("checkpoint")
    });
    assert!(
        matches!(
            result,
            Err(MaterialReadError::Material(
                MaterialError::InvalidSourceLimits
            ))
        ),
        "invalid configuration wins before source inspection or interruption"
    );
    assert_eq!(
        calls.get(),
        0,
        "invalid limits do not invoke the caller checkpoint"
    );
    let result = MaterialLibrary::parse_with(
        "synthetic/checkpoint",
        b"not XML",
        MaterialSourceLimits::default(),
        || Err("checkpoint"),
    );
    assert!(
        matches!(result, Err(MaterialReadError::Interrupted("checkpoint"))),
        "the caller interruption stays typed"
    );
    let result = MaterialLibrary::parse_with(
        "synthetic/root",
        b"<Other/>",
        MaterialSourceLimits::default(),
        || Ok::<(), Infallible>(()),
    );
    assert!(
        matches!(
            result,
            Err(MaterialReadError::Material(
                MaterialError::NotMaterial { .. }
            ))
        ),
        "the material-root contract remains unchanged"
    );
}

pub(crate) fn stencil_samples() -> Vec<Vec<u8>> {
    let source = TestElement::new("mAtErIaL")
        .child(TestElement::new("Ignored"))
        .child(
            TestElement::new("SUBMATERIALS")
                .child(TestElement::new("MatRef"))
                .child(
                    TestElement::new("mAtErIaL")
                        .attribute("Shader", "MeshDecal")
                        .attribute("StringGenMask", "STENCIL_MAP")
                        .child(TestElement::new("Unknown"))
                        .child(
                            TestElement::new("Textures")
                                .child(TestElement::new("Unknown"))
                                .child(
                                    TestElement::new("Texture")
                                        .attribute("Map", "TexSlot7")
                                        .attribute("File", ""),
                                )
                                .child(
                                    TestElement::new("Texture")
                                        .attribute("Map", "TexSlot1")
                                        .attribute("File", "synthetic/detail_detail.tif"),
                                )
                                .child(
                                    TestElement::new("Texture")
                                        .attribute("Map", "TexSlot7")
                                        .attribute("File", "synthetic/stencil.tif"),
                                ),
                        )
                        .child(
                            TestElement::new("PublicParams")
                                .attribute("StencilOpacity", "0")
                                .attribute("stenciltintoverride", "2"),
                        )
                        .child(
                            TestElement::new("PublicParams").attribute("StencilTintOverride", "1"),
                        ),
                ),
        );
    vec![
        source.to_xml().into_bytes(),
        source.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
    ]
}

#[test]
fn material_observer_baseline_keeps_original_stencil_inputs_and_parameter_precedence() -> TestResult
{
    for bytes in stencil_samples() {
        let library = MaterialLibrary::parse("synthetic/observer", &bytes)?;
        assert_eq!(
            library.unknown_entries, 1,
            "case-insensitive material lists keep unknown sibling IDs"
        );
        let material = library.materials.get(1).ok_or("typed stencil material")?;
        assert_eq!(
            material.textures.len(),
            2,
            "unknown and empty-file texture elements are not typed slots"
        );
        assert_eq!(
            material.public_params.len(),
            3,
            "separate PublicParams lists retain their authored order"
        );
        assert_eq!(
            material.palette,
            Some(PaletteSlot::Decal),
            "the first case-insensitive tint override wins"
        );
        assert_eq!(
            material.stencil_motif().ok_or("stencil motif")?.opacity,
            0.0,
            "stencil opacity remains part of the public motif result"
        );
    }
    Ok(())
}
