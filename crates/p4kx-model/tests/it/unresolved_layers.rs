// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The unresolved layer files counted across the libraries of a scene.

use std::error::Error;

use p4kx_model::{MaterialLibrary, MaterialStatus, UnresolvedLayers};
use p4kx_testkit::{TestMtl, TestMtlMaterial};

type TestResult = Result<(), Box<dyn Error>>;

fn status(unresolved: &[&str]) -> Result<MaterialStatus, Box<dyn Error>> {
    let bytes = TestMtl::single(&TestMtlMaterial::new("hull")).cryxml();
    let mut library = MaterialLibrary::parse("lib", &bytes)?;
    library.unresolved_layers = unresolved.iter().map(|path| (*path).to_owned()).collect();
    Ok(MaterialStatus::Loaded(library))
}

#[test]
fn spellings_of_one_path_across_libraries_are_one_file() -> TestResult {
    let statuses = [
        status(&["A\\b.mtl"])?,
        status(&["a/B.mtl"])?,
        status(&["Data\\a\\b"])?,
    ];

    let all = UnresolvedLayers::of(&statuses);

    assert_eq!(all.count(), 1, "also with Data/ and without .mtl");
    assert_eq!(all.first(), Some("A\\b.mtl"), "the first spelling is kept");
    Ok(())
}

#[test]
fn one_path_under_two_submaterials_counts_twice_as_the_library_lists_it() -> TestResult {
    let statuses = [status(&["a.mtl", "a.mtl"])?];

    assert_eq!(UnresolvedLayers::of(&statuses).count(), 2);
    Ok(())
}

#[test]
fn another_library_listing_the_same_entries_adds_none() -> TestResult {
    let statuses = [status(&["a.mtl", "a.mtl"])?, status(&["A.MTL", "b.mtl"])?];

    assert_eq!(UnresolvedLayers::of(&statuses).count(), 3);
    Ok(())
}

#[test]
fn statuses_without_a_library_add_nothing() {
    let statuses = [MaterialStatus::Unnamed, MaterialStatus::Skipped];

    let all = UnresolvedLayers::of(&statuses);

    assert_eq!((all.count(), all.first()), (0, None));
}
