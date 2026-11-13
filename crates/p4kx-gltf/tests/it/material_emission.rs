// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{CheckedGlb, TestMtl};

use crate::materials::{close, export, material_of, matte};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn emissive_is_scaled_by_glow_and_kept_in_range() -> TestResult {
    let lit = matte("lit").emissive([0.5, 0.25, 0.0]).glow(1.0);
    let bright = matte("bright").emissive([1.0, 0.5, 0.0]).glow(4.0);
    let file = TestMtl::multi(&[matte("unused"), lit, bright]).cryxml();

    let (glb, _) = export(Some(file))?;
    let extension = "KHR_materials_emissive_strength";
    let root = glb.document.as_json();
    assert_eq!(
        root.extensions_used,
        [extension],
        "declare the extension once"
    );
    assert!(
        root.extensions_required.is_empty(),
        "core fallback remains usable"
    );
    assert_eq!(emissive_strength(&glb, "lit")?, None);
    assert_eq!(emissive_strength(&glb, "bright")?, Some(4.0));

    let lit = glb
        .document
        .materials()
        .nth(material_of(&glb, "lit")?)
        .ok_or("lit")?;
    assert!(
        close(&lit.emissive_factor(), &[0.5, 0.25, 0.0]),
        "emissive times glow"
    );
    let bright = glb
        .document
        .materials()
        .nth(material_of(&glb, "bright")?)
        .ok_or("bright")?;
    assert!(
        close(&bright.emissive_factor(), &[1.0, 0.5, 0.0]),
        "above 1 it is normalised: {:?}",
        bright.emissive_factor()
    );
    Ok(())
}

fn emissive_strength(glb: &CheckedGlb, name: &str) -> Result<Option<f32>, Box<dyn Error>> {
    Ok(glb
        .document
        .as_json()
        .materials
        .get(material_of(glb, name)?)
        .and_then(|material| material.extensions.as_ref())
        .and_then(|extensions| extensions.emissive_strength.as_ref())
        .map(|strength| strength.emissive_strength.0))
}

#[test]
fn emissive_magnitude_remains_distinct_and_ignores_nonfinite_input() -> TestResult {
    let (glb, _) = export(Some(
        TestMtl::multi(&[
            matte("unused"),
            matte("stronger").emissive([1.0, 0.5, 0.0]).glow(8.0),
            matte("invalid")
                .emissive([f32::NAN, f32::INFINITY, -1.0])
                .glow(4.0),
        ])
        .cryxml(),
    ))?;
    assert_eq!(emissive_strength(&glb, "stronger")?, Some(8.0));
    assert_eq!(emissive_strength(&glb, "invalid")?, None);
    let (glb, _) = export(Some(
        TestMtl::multi(&[
            matte("unused"),
            matte("dark").emissive([1.0; 3]),
            matte("x"),
        ])
        .cryxml(),
    ))?;
    assert_eq!(glb.document.as_json().extensions_used, [] as [String; 0]);
    let dark = glb
        .document
        .materials()
        .nth(material_of(&glb, "dark")?)
        .ok_or("dark")?;
    assert!(
        close(&dark.emissive_factor(), &[0.0; 3]),
        "no glow, no emission"
    );
    Ok(())
}
