// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::MaterialLibrary;
use p4kx_testkit::{TestElement, TestMtl, TestMtlMaterial};
use serde_json::{Value, json};

use crate::complete_material_inputs_source;
use crate::exact_material_assertions as check;
use crate::exact_material_fixture::{self as fixture, Encoding};
use crate::support::TestResult;

const BEYOND_RAW_ELEMENTS: usize = 4097;

#[test]
fn complete_material_empty_layers_survive_the_independent_raw_element_budget() -> TestResult {
    let material = (0..BEYOND_RAW_ELEMENTS).fold(TestMtlMaterial::new("empty"), |material, _| {
        material.layer_with(&[])
    });
    let install = fixture::install(&TestMtl::single(&material), Encoding::CryXml, Vec::new())?;
    complete_material_inputs_source::with_original(&install, |_, root| {
        let layers = root
            .children()
            .find(|child| child.name() == "MatLayers")
            .ok_or("original layer list")?;
        let layers: Vec<_> = layers.children().collect();
        assert_eq!(
            layers.len(),
            BEYOND_RAW_ELEMENTS,
            "all original empty-layer occurrences"
        );
        for layer in layers {
            assert_eq!(layer.name(), "Layer", "actual original layer tag");
            assert_eq!(
                layer.attributes().count(),
                0,
                "all actual source fields are absent"
            );
        }
        Ok(())
    })?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let output = server.ok("mtl_summary", &json!({"path":fixture::MATERIAL}))?;
    assert!(
        serde_json::to_vec(&output)?.len() < 256 << 10,
        "the complete empty-field DTO list fits"
    );
    let material = check::material(&output)?;
    assert_eq!(
        material.get("layer_count"),
        Some(&json!(BEYOND_RAW_ELEMENTS)),
        "no hidden source count cut"
    );
    let layers = material
        .get("layers")
        .and_then(Value::as_array)
        .ok_or("all layers")?;
    assert_eq!(
        layers.len(),
        BEYOND_RAW_ELEMENTS,
        "all original occurrence slots"
    );
    for layer in layers {
        assert_eq!(
            layer,
            &json!({}),
            "all absent source attributes remain omitted"
        );
    }
    Ok(())
}

#[test]
fn complete_material_unrelated_raw_truncation_does_not_refuse_complete_layers() -> TestResult {
    let unknown = (0..BEYOND_RAW_ELEMENTS)
        .fold(TestElement::new("UnknownControl"), |element, _| {
            element.child(TestElement::new("Leaf"))
        });
    let material = TestMtlMaterial::new("known")
        .layer_with(&[("Name", "kept"), ("Path", "objects/kept.mtl")])
        .child(unknown);
    let install = fixture::install(&TestMtl::single(&material), Encoding::CryXml, Vec::new())?;
    complete_material_inputs_source::with_original(&install, |bytes, root| {
        let unknown = root
            .children()
            .find(|child| child.name() == "UnknownControl")
            .ok_or("original unknown block")?;
        assert_eq!(
            unknown.children().count(),
            BEYOND_RAW_ELEMENTS,
            "actual unrelated original raw children"
        );
        check_original_complete_layer(bytes)?;
        Ok(())
    })?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let output = server.ok("mtl_summary", &json!({"path":fixture::MATERIAL}))?;
    let material = check::material(&output)?;
    assert_eq!(
        material.get("layer_count"),
        Some(&json!(1)),
        "no unrelated blanket refusal"
    );
    assert_eq!(
        material.get("layers"),
        Some(&json!([{"name":"kept","path":"objects/kept.mtl"}])),
        "all actual advertised fields remain available"
    );
    Ok(())
}

fn check_original_complete_layer(bytes: &[u8]) -> TestResult {
    let original = MaterialLibrary::parse(fixture::MATERIAL, bytes)?;
    let material = original
        .materials
        .first()
        .ok_or("actual original interpreted material")?;
    assert!(
        material
            .truncated
            .iter()
            .any(|reason| reason == "elements beyond 4096"),
        "the original raw-element guard is actually reached"
    );
    assert_eq!(
        material.layers.len(),
        1,
        "advertised layer group remains complete"
    );
    let layer = material.layers.first().ok_or("actual original layer")?;
    assert_eq!(
        layer.attribute("Name"),
        Some("kept"),
        "original advertised value"
    );
    assert_eq!(
        layer.attribute("Path"),
        Some("objects/kept.mtl"),
        "original advertised path"
    );
    Ok(())
}
