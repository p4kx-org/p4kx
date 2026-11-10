// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The parts of a `.mtl` material the reader keeps as the file has them: layers, shader
//! parameters, texture slot attributes and modifiers, other attributes and elements.

use std::error::Error;

use p4kx_model::{MAX_RAW_DEPTH, MAX_RAW_ELEMENTS, MaterialError, MaterialLibrary, PaletteSlot};

use p4kx_testkit::{TestElement, TestMtl, TestMtlMaterial};

type TestResult = Result<(), Box<dyn Error>>;

fn parse(file: &TestMtl) -> Result<MaterialLibrary, MaterialError> {
    MaterialLibrary::parse("objects/test/raw", &file.cryxml())
}

fn pair(name: &str, value: &str) -> (String, String) {
    (name.to_owned(), value.to_owned())
}

#[test]
fn layers_keep_all_their_attributes_in_file_order() -> TestResult {
    let material = TestMtlMaterial::new("hull").layer_with(&[
        ("Name", "Primary"),
        ("Path", "a/b.mtl"),
        ("TintColor", "1,0,0"),
        ("PaletteTint", "2"),
        ("Extra", "x"),
    ]);

    let library = parse(&TestMtl::single(&material))?;

    let layer = library
        .materials
        .first()
        .and_then(|material| material.layers.first())
        .ok_or("layer")?;
    let names: Vec<&str> = layer
        .attributes
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, ["Name", "Path", "TintColor", "PaletteTint", "Extra"]);
    assert_eq!(layer.attribute("path"), Some("a/b.mtl"), "ignoring case");
    assert_eq!(layer.palette_slot(), Some(PaletteSlot::Secondary));
    Ok(())
}

#[test]
fn shader_parameters_and_unknown_attributes_and_elements_are_kept() -> TestResult {
    let material = TestMtlMaterial::new("odd")
        .attribute("Strange", "1")
        .attribute("Diffuse", "0.5,0.5,0.5")
        .public_param("A", "1")
        .public_param("B", "two")
        .child(TestElement::new("Extra").attribute("K", "v"));

    let library = parse(&TestMtl::single(&material))?;

    let odd = library.materials.first().ok_or("material")?;
    assert_eq!(odd.public_params, [pair("A", "1"), pair("B", "two")]);
    assert_eq!(odd.other_attributes, [pair("Strange", "1")]);
    let [extra] = odd.elements.as_slice() else {
        return Err("one element".into());
    };
    assert_eq!(extra.name, "Extra");
    assert_eq!(extra.attributes, [pair("K", "v")]);
    Ok(())
}

#[test]
fn a_public_colour_has_exactly_three_finite_components() -> TestResult {
    let material = TestMtlMaterial::new("params")
        .public_param("Colour", "0.1, 0.2, 0.3")
        .public_param("Number", "0.5")
        .public_param("Pair", "1,2")
        .public_param("Four", "1,2,3,4")
        .public_param("Infinite", "1,inf,3");

    let library = parse(&TestMtl::single(&material))?;

    let params = library.materials.first().ok_or("material")?;
    assert_eq!(params.public_colour("colour"), Some([0.1, 0.2, 0.3]));
    assert_eq!(params.public_param("Number"), Some([0.5, 0.0, 0.0]));
    for name in ["Number", "Pair", "Four", "Infinite", "Missing"] {
        assert_eq!(params.public_colour(name), None, "{name}");
    }
    Ok(())
}

#[test]
fn a_texture_slot_keeps_its_attributes_and_modifier() -> TestResult {
    let material = TestMtlMaterial::new("tex").texture_with_mod(
        "TexSlot1",
        "a/b.tif",
        &[("TileU", "2"), ("OffsetV", "0.5")],
    );

    let library = parse(&TestMtl::single(&material))?;

    let slot = library
        .materials
        .first()
        .and_then(|material| material.textures.first())
        .ok_or("slot")?;
    assert_eq!(slot.attributes, [pair("TexType", "0"), pair("Used", "1")]);
    let [tex_mod] = slot.children.as_slice() else {
        return Err("one child".into());
    };
    assert_eq!(tex_mod.name, "TexMod");
    assert_eq!(
        tex_mod.attributes,
        [pair("TileU", "2"), pair("OffsetV", "0.5")]
    );
    Ok(())
}

#[test]
fn layers_are_complete_and_raw_element_limits_are_reported() -> TestResult {
    const LAYERS: usize = 65;
    let layered = (0..LAYERS).fold(TestMtlMaterial::new("many"), |material, _| {
        material.layer_with(&[("Name", "L")])
    });
    let library = parse(&TestMtl::single(&layered))?;
    let many = library.materials.first().ok_or("material")?;
    assert_eq!(many.layers.len(), LAYERS, "all original layer occurrences");
    assert!(
        many.truncated.is_empty(),
        "typed layers do not exhaust raw fragment storage"
    );

    let crowded = (0..=MAX_RAW_ELEMENTS).fold(TestMtlMaterial::new("crowd"), |material, _| {
        material.child(TestElement::new("E"))
    });
    let library = parse(&TestMtl::single(&crowded))?;
    let crowd = library.materials.first().ok_or("material")?;
    assert_eq!(crowd.elements.len(), MAX_RAW_ELEMENTS);
    assert_eq!(
        crowd.truncated,
        [format!("elements beyond {MAX_RAW_ELEMENTS}")]
    );
    Ok(())
}

#[test]
fn raw_elements_stop_at_the_depth_limit() -> TestResult {
    let deep = (0..MAX_RAW_DEPTH + 4).fold(TestElement::new("Leaf"), |inner, _| {
        TestElement::new("Node").child(inner)
    });
    let material = TestMtlMaterial::new("deep").child(deep);

    let library = parse(&TestMtl::single(&material))?;

    let mut depth = 0;
    let mut element = library
        .materials
        .first()
        .and_then(|material| material.elements.first())
        .ok_or("element")?;
    while let Some(child) = element.children.first() {
        depth += 1;
        element = child;
    }
    assert_eq!(depth, MAX_RAW_DEPTH - 1);
    Ok(())
}
