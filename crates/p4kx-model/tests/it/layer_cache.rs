// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The layer files shared by the loads of several libraries, their limits, and the plain read
//! that leaves them unread.

use std::cell::Cell;
use std::error::Error;

use p4kx_chunks::ChunkFile;
use p4kx_model::{
    CompanionError, CompanionPath, LayerCache, LayerLimits, LodSelection, MaterialLibrary,
    MaterialReading, MaterialStatus, Model, PositionBox,
};
use p4kx_testkit::{TestModel, TestMtl, TestMtlMaterial};

type TestResult = Result<(), Box<dyn Error>>;

const FIRST: &str = "objects/test/first";
const SECOND: &str = "objects/test/second";
const SET: &str = "materials/layers/set";

fn hull(submaterial: &str) -> Vec<u8> {
    layering(&[("materials/layers/set.mtl", submaterial)])
}

/// Returns a library of one material with a layer for each of the `(path, Submtl)` pairs.
fn layering(layers: &[(&str, &str)]) -> Vec<u8> {
    let material = layers
        .iter()
        .fold(TestMtlMaterial::new("hull"), |material, (path, name)| {
            material.layer_with(&[("Name", "L"), ("Path", path), ("Submtl", name)])
        });
    TestMtl::single(&material).cryxml()
}

fn set() -> Vec<u8> {
    TestMtl::multi(&[
        TestMtlMaterial::new("first").diffuse([0.1; 3]),
        TestMtlMaterial::new("second").diffuse([0.9; 3]),
    ])
    .cryxml()
}

/// Returns the diffuse colour of the surface of each layer of the first material.
fn diffuse_of(status: &MaterialStatus) -> Result<Vec<Option<[f32; 3]>>, Box<dyn Error>> {
    let library = status.library().ok_or("loaded")?;
    let material = library.materials.first().ok_or("material")?;
    Ok(material
        .layers
        .iter()
        .map(|layer| layer.surface.as_ref().map(|surface| surface.diffuse))
        .collect())
}

fn load(
    name: &str,
    companions: &mut dyn p4kx_model::CompanionResolver,
    cache: &LayerCache,
) -> MaterialStatus {
    MaterialStatus::load_with(Some(name), companions, MaterialReading::Drawn, cache)
}

#[test]
fn a_layer_file_shared_by_two_libraries_is_fetched_once() -> TestResult {
    let fetched = Cell::new(0);
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(FIRST) => Some(hull("first")),
            CompanionPath::Material(SECOND) => Some(hull("second")),
            CompanionPath::Material(SET) => {
                fetched.set(fetched.get() + 1);
                Some(set())
            }
            _ => None,
        })
    };
    let cache = LayerCache::new();

    let first = load(FIRST, &mut companions, &cache);
    let second = load(SECOND, &mut companions, &cache);

    assert_eq!(diffuse_of(&first)?, [Some([0.1; 3])], "first");
    assert_eq!(
        diffuse_of(&second)?,
        [Some([0.9; 3])],
        "another sub-material"
    );
    assert_eq!(fetched.get(), 1, "one fetch and one parse for both");
    assert_eq!(cache.len(), 1, "one file kept");
    Ok(())
}

#[test]
fn without_a_shared_cache_each_load_fetches_the_file() {
    let fetched = Cell::new(0);
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(FIRST) => Some(hull("first")),
            CompanionPath::Material(SET) => {
                fetched.set(fetched.get() + 1);
                Some(set())
            }
            _ => None,
        })
    };

    MaterialStatus::load(Some(FIRST), &mut companions);
    MaterialStatus::load(Some(FIRST), &mut companions);

    assert_eq!(fetched.get(), 2);
}

#[test]
fn one_load_stops_fetching_once_its_bytes_are_reached_and_the_skipped_file_is_retried() -> TestResult
{
    let layer = set();
    let limits = LayerLimits {
        bytes_per_load: layer.len(),
        ..LayerLimits::default()
    };
    let fetched = Cell::new(0);
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(FIRST) => Some(layering(&[
                ("materials/layers/a.mtl", "first"),
                ("materials/layers/b.mtl", "first"),
            ])),
            CompanionPath::Material(SECOND) => Some(layering(&[("materials/layers/b.mtl", "")])),
            CompanionPath::Material(_) => {
                fetched.set(fetched.get() + 1);
                Some(layer.clone())
            }
            _ => None,
        })
    };
    let cache = LayerCache::with_limits(limits);

    let first = load(FIRST, &mut companions, &cache);

    let library = first.library().ok_or("loaded")?;
    assert_eq!(fetched.get(), 1, "the second file is over the budget");
    assert_eq!(diffuse_of(&first)?, [Some([0.1; 3]), None]);
    assert_eq!(library.unresolved_layers, ["materials/layers/b.mtl"]);
    assert_eq!(cache.len(), 1, "the skipped file is not cached");

    let second = load(SECOND, &mut companions, &cache);

    assert_eq!(fetched.get(), 2, "the next load fetches it");
    assert_eq!(diffuse_of(&second)?, [Some([0.1; 3])]);
    Ok(())
}

#[test]
fn which_layers_resolve_does_not_depend_on_what_the_cache_holds() -> TestResult {
    let limits = LayerLimits {
        files_per_load: 1,
        ..LayerLimits::default()
    };
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(FIRST) => Some(layering(&[
                ("materials/layers/a.mtl", ""),
                ("materials/layers/b.mtl", ""),
            ])),
            CompanionPath::Material(_) => Some(set()),
            _ => None,
        })
    };
    let cold = LayerCache::with_limits(limits);
    let warm = LayerCache::with_limits(limits);
    let mut warming = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(SECOND) => Some(layering(&[("materials/layers/b.mtl", "")])),
            CompanionPath::Material(_) => Some(set()),
            _ => None,
        })
    };
    load(SECOND, &mut warming, &warm);
    assert_eq!(warm.len(), 1, "b is cached");

    let from_cold = load(FIRST, &mut companions, &cold);
    let from_warm = load(FIRST, &mut companions, &warm);

    assert_eq!(diffuse_of(&from_cold)?, diffuse_of(&from_warm)?);
    assert_eq!(diffuse_of(&from_warm)?, [Some([0.1; 3]), None], "a, not b");
    Ok(())
}

#[test]
fn a_full_cache_starts_over() {
    let fetched = Cell::new(0);
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(FIRST) => Some(layering(&[("materials/layers/a.mtl", "")])),
            CompanionPath::Material(SECOND) => Some(layering(&[("materials/layers/b.mtl", "")])),
            CompanionPath::Material(_) => {
                fetched.set(fetched.get() + 1);
                Some(set())
            }
            _ => None,
        })
    };
    let cache = LayerCache::with_limits(LayerLimits {
        cached_files: 1,
        ..LayerLimits::default()
    });

    load(FIRST, &mut companions, &cache);
    load(SECOND, &mut companions, &cache);
    assert_eq!(cache.len(), 1, "a made room for b");
    load(FIRST, &mut companions, &cache);

    assert_eq!(fetched.get(), 3, "a was fetched again");
    assert!(!cache.is_empty());
}

#[test]
fn a_plain_load_reads_no_layer_file() -> TestResult {
    let fetched = Cell::new(0);
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(FIRST) => Some(hull("first")),
            CompanionPath::Material(_) => {
                fetched.set(fetched.get() + 1);
                Some(set())
            }
            _ => None,
        })
    };

    let status = MaterialStatus::load_with(
        Some(FIRST),
        &mut companions,
        MaterialReading::Plain,
        &LayerCache::new(),
    );

    let library = status.library().ok_or("loaded")?;
    assert_eq!(fetched.get(), 0, "nothing is asked for");
    assert!(library.unresolved_layers.is_empty(), "nothing is reported");
    assert_eq!(diffuse_of(&status)?, [None], "no surface");
    Ok(())
}

#[test]
fn a_skipped_load_reads_nothing() {
    let mut companions =
        |_: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> { Ok(None) };

    let status = MaterialStatus::load_with(
        Some(FIRST),
        &mut companions,
        MaterialReading::Skipped,
        &LayerCache::new(),
    );

    assert_eq!(status, MaterialStatus::Skipped);
}

#[test]
fn a_model_reads_plain_materials_without_layer_files_and_drawn_ones_with() -> TestResult {
    let cube = TestModel::cube().with_material_library(FIRST);
    let (cgfm, library) = (cube.cgfm(), hull("first"));
    let file = ChunkFile::parse(cube.cgf())?;
    let fetched = Cell::new(0);
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Sibling("cube.cgfm") => Some(cgfm.clone()),
            CompanionPath::Material(FIRST) => Some(library.clone()),
            CompanionPath::Material(_) => {
                fetched.set(fetched.get() + 1);
                Some(set())
            }
            _ => None,
        })
    };
    let read = |companions: &mut dyn p4kx_model::CompanionResolver, reading| {
        Model::read_with_materials(
            "cube.cgf",
            &file,
            companions,
            LodSelection::All,
            PositionBox::default(),
            reading,
        )
    };

    let plain = read(&mut companions, MaterialReading::Plain)?;
    assert_eq!(fetched.get(), 0, "no layer file for a listing");
    assert_eq!(diffuse_of(&plain.materials)?, [None], "no surface");

    let skipped = read(&mut companions, MaterialReading::Skipped)?;
    assert_eq!(skipped.materials, MaterialStatus::Skipped, "not read");

    let drawn = read(&mut companions, MaterialReading::Drawn)?;
    assert_eq!(fetched.get(), 1, "one layer file for drawing");
    assert_eq!(diffuse_of(&drawn.materials)?, [Some([0.1; 3])], "surface");
    Ok(())
}

#[test]
fn the_sub_material_of_a_layer_is_picked_by_one_rule() -> TestResult {
    let library = MaterialLibrary::parse(SET, &set())?;
    assert_eq!(
        library.layer_position(""),
        Some(0),
        "the first without a name"
    );
    assert_eq!(
        library.layer_position(" Second "),
        Some(1),
        "by name, trimmed"
    );
    assert_eq!(library.layer_position("absent"), None, "no such one");
    let single = MaterialLibrary::parse(
        SET,
        &TestMtl::single(&TestMtlMaterial::new("only")).cryxml(),
    )?;
    assert_eq!(single.layer_position("anything"), Some(0), "a single file");
    Ok(())
}

#[test]
fn a_layer_file_read_as_a_library_is_fetched_once_with_the_surface_loads() -> TestResult {
    let fetched = Cell::new(0);
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material(FIRST) => Some(hull("first")),
            CompanionPath::Material(SET) => {
                fetched.set(fetched.get() + 1);
                Some(set())
            }
            _ => None,
        })
    };
    let cache = LayerCache::new();
    let mut budget = cache.budget();

    load(FIRST, &mut companions, &cache);
    let status = cache.library(
        r"Data\materials\layers\SET.mtl",
        &mut companions,
        &mut budget,
    );
    let again = cache.library(SET, &mut companions, &mut budget);

    let library = status.library().ok_or("loaded")?;
    assert_eq!(library.materials.len(), 2, "every sub-material is there");
    assert!(std::sync::Arc::ptr_eq(&status, &again), "one parsed file");
    assert_eq!(fetched.get(), 1, "one fetch for the load and both reads");
    Ok(())
}

#[test]
fn a_layer_library_that_cannot_be_read_is_a_status_with_its_reason() {
    let mut companions = |path: CompanionPath<'_>| -> Result<Option<Vec<u8>>, CompanionError> {
        Ok(match path {
            CompanionPath::Material("materials/layers/broken") => Some(b"not xml".to_vec()),
            _ => None,
        })
    };
    let cache = LayerCache::with_limits(LayerLimits {
        files_per_load: 2,
        ..LayerLimits::default()
    });
    let mut budget = cache.budget();

    let missing = cache.library("materials/layers/gone.mtl", &mut companions, &mut budget);
    let broken = cache.library("materials/layers/broken.mtl", &mut companions, &mut budget);
    let spent = cache.library("materials/layers/third.mtl", &mut companions, &mut budget);
    let unsafe_path = cache.library("../outside.mtl", &mut companions, &mut budget);

    assert!(matches!(*missing, MaterialStatus::Missing { .. }));
    assert!(matches!(*broken, MaterialStatus::Unreadable { .. }));
    assert!(spent.problem().is_some_and(|text| text.contains("budget")));
    assert!(matches!(*unsafe_path, MaterialStatus::Unreadable { .. }));
    assert_eq!(
        cache.len(),
        2,
        "the unadmitted and unsafe files are not cached"
    );
}
