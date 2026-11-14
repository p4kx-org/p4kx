// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The surfaces of the layer files of a material library, fetched through the companions.

use std::cell::Cell;
use std::collections::HashMap;
use std::error::Error;

use p4kx_model::{
    CompanionError, CompanionPath, MAX_LAYER_FILES, MaterialLibrary, MaterialStatus, PaletteSlot,
    TintEntry, TintPalette,
};
use p4kx_testkit::{TestMtl, TestMtlMaterial};

type TestResult = Result<(), Box<dyn Error>>;

const LIBRARY: &str = "objects/test/ship";

fn layer_file(diffuse: [f32; 3], specular: [f32; 3], shininess: f32) -> Vec<u8> {
    TestMtl::single(
        &TestMtlMaterial::new("layer")
            .shader("Layer")
            .diffuse(diffuse)
            .specular(specular)
            .shininess(shininess),
    )
    .cryxml()
}

fn load(
    library: &TestMtl,
    files: &HashMap<&'static str, Vec<u8>>,
    fetched: &Cell<usize>,
) -> MaterialStatus {
    let library = library.cryxml();
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(LIBRARY) => Some(library.clone()),
            CompanionPath::Material(other) => {
                fetched.set(fetched.get() + 1);
                files.get(other).cloned()
            }
            _ => None,
        })
    };
    MaterialStatus::load(Some(LIBRARY), &mut companions)
}

fn hull() -> TestMtlMaterial {
    TestMtlMaterial::new("hull")
        .shader("HardSurface")
        .layer_with(&[
            ("Name", "Primary"),
            ("Path", "materials/layers/base.mtl"),
            ("TintColor", "0.5,0.25,1"),
            ("GlossMult", "0.5"),
            ("PaletteTint", "1"),
        ])
        .layer_with(&[
            ("Name", "Wear"),
            ("Path", "Data/Materials/Layers/Wear.mtl"),
            ("TintColor", "x"),
        ])
        .public_param("WearBlendBase", "0.25")
}

#[test]
fn the_files_of_the_layers_are_read_and_their_surfaces_kept() -> TestResult {
    let files = HashMap::from([
        (
            "materials/layers/base",
            layer_file([1.0; 3], [0.1, 0.2, 0.3], 200.0),
        ),
        (
            "Materials/Layers/Wear",
            layer_file([0.4; 3], [0.04; 3], 255.0),
        ),
    ]);
    let fetched = Cell::new(0);

    let status = load(&TestMtl::single(&hull()), &files, &fetched);

    let library = status.library().ok_or("loaded")?;
    let material = library.materials.first().ok_or("material")?;
    let [primary, wear] = material.layers.as_slice() else {
        return Err("two layers".into());
    };
    let surface = primary.surface.as_ref().ok_or("surface")?;
    assert_eq!(surface.diffuse, [1.0; 3], "diffuse");
    assert_eq!(surface.specular, [0.1, 0.2, 0.3], "specular");
    assert!(wear.surface.is_some(), "a path below Data is read too");
    assert_eq!(primary.tint(), [0.5, 0.25, 1.0], "tint colour");
    assert_eq!(wear.tint(), [1.0; 3], "a malformed tint is white");
    assert!(
        (primary.gloss_mult() - 0.5).abs() < 1e-6,
        "gloss multiplier"
    );
    assert!((material.wear_blend() - 0.25).abs() < 1e-6, "wear blend");
    assert!(library.unresolved_layers.is_empty(), "nothing is missing");
    Ok(())
}

#[test]
fn a_missing_layer_file_is_listed_once_and_leaves_the_layer_without_a_surface() -> TestResult {
    let files = HashMap::from([(
        "materials/layers/base",
        layer_file([1.0; 3], [0.04; 3], 255.0),
    )]);
    let second = hull().layer_with(&[
        ("Name", "Again"),
        ("Path", "Data/Materials/Layers/Wear.mtl"),
    ]);
    let fetched = Cell::new(0);

    let status = load(&TestMtl::multi(&[hull(), second]), &files, &fetched);

    let library = status.library().ok_or("loaded")?;
    assert_eq!(
        library.unresolved_layers,
        ["Data/Materials/Layers/Wear.mtl"],
        "reported once"
    );
    let missing = library
        .materials
        .iter()
        .flat_map(|material| &material.layers)
        .filter(|layer| layer.surface.is_none())
        .count();
    assert_eq!(missing, 3, "every layer of the missing file");
    assert_eq!(fetched.get(), 2, "each file is fetched once");
    Ok(())
}

#[test]
fn a_sub_material_of_a_layer_file_is_chosen_by_name() -> TestResult {
    let file = TestMtl::multi(&[
        TestMtlMaterial::new("first").diffuse([0.1; 3]),
        TestMtlMaterial::new("Second").diffuse([0.9; 3]),
    ])
    .cryxml();
    let material = TestMtlMaterial::new("hull")
        .layer_with(&[
            ("Name", "Primary"),
            ("Path", "materials/layers/set.mtl"),
            ("Submtl", "second"),
        ])
        .layer_with(&[
            ("Name", "Wear"),
            ("Path", "materials/layers/set.mtl"),
            ("Submtl", "absent"),
        ]);
    let files = HashMap::from([("materials/layers/set", file)]);

    let status = load(&TestMtl::single(&material), &files, &Cell::new(0));

    let library = status.library().ok_or("loaded")?;
    let layers = &library.materials.first().ok_or("material")?.layers;
    let diffuse: Vec<Option<[f32; 3]>> = layers
        .iter()
        .map(|layer| layer.surface.as_ref().map(|surface| surface.diffuse))
        .collect();
    assert_eq!(diffuse, [Some([0.9; 3]), None], "by name, else unresolved");
    assert_eq!(library.unresolved_layers.len(), 1, "the absent one");
    Ok(())
}

#[test]
fn spellings_of_one_missing_file_are_listed_once() -> TestResult {
    let material = TestMtlMaterial::new("hull")
        .layer_with(&[("Name", "A"), ("Path", "Materials/Layers/Lost.mtl")])
        .layer_with(&[("Name", "B"), ("Path", "materials\\layers\\lost.mtl")]);

    let status = load(&TestMtl::single(&material), &HashMap::new(), &Cell::new(0));

    let library = status.library().ok_or("loaded")?;
    assert_eq!(library.unresolved_layers.len(), 1, "one file");
    Ok(())
}

#[test]
fn a_path_that_leaves_the_data_folder_is_not_fetched() -> TestResult {
    let material =
        TestMtlMaterial::new("hull").layer_with(&[("Name", "Primary"), ("Path", "../secret.mtl")]);
    let fetched = Cell::new(0);

    let status = load(&TestMtl::single(&material), &HashMap::new(), &fetched);

    let library = status.library().ok_or("loaded")?;
    assert_eq!(fetched.get(), 0, "nothing is asked for");
    assert_eq!(library.unresolved_layers, ["../secret.mtl"], "reported");
    Ok(())
}

#[test]
fn the_files_one_library_makes_the_reader_fetch_are_limited() -> TestResult {
    const MATERIAL_GROUPS_BEFORE_LIMIT: usize = 4;
    let layers_per_material = MAX_LAYER_FILES / MATERIAL_GROUPS_BEFORE_LIMIT;
    let materials = MAX_LAYER_FILES / layers_per_material + 2;
    let paths: Vec<String> = (0..materials * layers_per_material)
        .map(|at| format!("materials/layers/l{at}.mtl"))
        .collect();
    let list: Vec<TestMtlMaterial> = paths
        .chunks(layers_per_material)
        .enumerate()
        .map(|(at, chunk)| {
            chunk
                .iter()
                .fold(TestMtlMaterial::new(format!("m{at}")), |material, path| {
                    material.layer_with(&[("Name", "L"), ("Path", path)])
                })
        })
        .collect();
    let fetched = Cell::new(0);

    let status = load(&TestMtl::multi(&list), &HashMap::new(), &fetched);

    let library = status.library().ok_or("loaded")?;
    assert_eq!(fetched.get(), MAX_LAYER_FILES, "the files asked for");
    assert_eq!(
        library.unresolved_layers.len(),
        MAX_LAYER_FILES,
        "the list is bounded too"
    );
    Ok(())
}

#[test]
fn a_palette_gives_each_layer_with_a_palette_tint_its_entry() -> TestResult {
    let material = TestMtlMaterial::new("hull")
        .layer_with(&[("Name", "Primary"), ("PaletteTint", "2")])
        .layer_with(&[("Name", "Wear"), ("PaletteTint", "0")])
        .layer_with(&[("Name", "Other"), ("PaletteTint", "3")]);
    let library = MaterialLibrary::parse(LIBRARY, &TestMtl::single(&material).cryxml())?;
    let entry = |value: f32| TintEntry {
        tint: [value; 3],
        specular: [0.04; 3],
        glossiness: 100.0,
    };
    let palette = TintPalette::new("p", [entry(0.1), entry(0.2), entry(0.3)], [0.5; 3]);
    let hull = library.materials.first().ok_or("material")?;
    assert_eq!(hull.palette, Some(PaletteSlot::Secondary));

    let tinted = palette.apply(hull).ok_or("tinted")?;

    let tints: Vec<Option<[f32; 3]>> = tinted
        .layers
        .iter()
        .map(|layer| layer.palette_entry.map(|entry| entry.tint))
        .collect();
    assert_eq!(tints, [Some([0.2; 3]), None, Some([0.3; 3])]);
    Ok(())
}
