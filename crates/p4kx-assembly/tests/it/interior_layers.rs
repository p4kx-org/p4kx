// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The layer files of the material libraries of an Interior's parts.

use p4kx_assembly::{Interior, InteriorOptions, IssueKind};
use p4kx_extraction::{Archive, select_entries};
use p4kx_testkit::{INTERIOR_ROOT, TestEntry, TestMtl, TestMtlMaterial, interior_entries};

use crate::assembly_support::{TestResult, fixture};

#[test]
fn a_missing_layer_file_of_a_library_is_an_issue_of_the_part() -> TestResult {
    let layered = TestMtlMaterial::new("hull")
        .shader("HardSurface")
        .layer_with(&[("Name", "Primary"), ("Path", "materials/layers/absent.mtl")]);
    let mut entries: Vec<TestEntry> = interior_entries()
        .into_iter()
        .filter(|entry| !format!("{entry:?}").contains("ship.mtl"))
        .collect();
    entries.push(TestEntry::stored(
        "Data\\objects\\test\\ship.mtl",
        TestMtl::multi(&[layered.clone(), layered.clone(), layered]).cryxml(),
    ));
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let found = select_entries(&archive, &[INTERIOR_ROOT])?;
    let entry = found.first().ok_or("no root container")?;
    let root = archive.open_container(entry)?;

    let interior = Interior::build(
        &[&archive, &root],
        INTERIOR_ROOT,
        &InteriorOptions::default(),
    )?;

    let reported = interior.issues().iter().any(|issue| {
        matches!(
            issue.kind(),
            IssueKind::MaterialLibrary { reason } if reason.contains("layer files")
        )
    });
    assert!(reported, "the missing layer file is reported");
    let unresolved = interior.unresolved_layers();
    assert_eq!(
        unresolved.count(),
        1,
        "named by three libraries, counted once"
    );
    assert_eq!(unresolved.first(), Some("materials/layers/absent.mtl"));
    Ok(())
}
