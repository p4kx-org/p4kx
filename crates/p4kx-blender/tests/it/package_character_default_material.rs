// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_testkit::{HEAD_VERTICES, MALE_RIG_HASH, check_glb, head_entries, head_rig};

use super::{TestResult, Written, mesh_file, write_with_entries};
use std::error::Error;

pub(super) fn write(expressions: bool, animated: bool) -> Result<Written, Box<dyn Error>> {
    write_with_entries(
        expressions,
        animated,
        head_entries(Some(head_rig(HEAD_VERTICES, MALE_RIG_HASH).build())),
    )
}

#[test]
fn physical_default_is_bound_to_the_rigged_package() -> TestResult {
    let mut entries = head_entries(Some(head_rig(HEAD_VERTICES, MALE_RIG_HASH).build()));
    entries.retain(|entry| {
        !Path::new(entry.path())
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mtl"))
    });
    let source = "Data/Materials/material_default.mtl";
    let material = br#"<Material Name="archive-fallback" Shader="Illum" Diffuse="0.1,0.2,0.3"/>"#;
    entries.push(p4kx_testkit::TestEntry::stored(source, material));
    let written = write_with_entries(false, false, entries)?;
    let receipt = written
        .manifest
        .default_material
        .ok_or("physical fallback receipt")?;
    assert_eq!(
        receipt.state,
        p4kx_extraction::DefaultMaterialState::Resolved,
        "verified fallback"
    );
    assert_eq!(
        receipt.source.ok_or("original source")?.entry_path,
        source,
        "actual Archive Entry"
    );
    assert!(
        written
            .manifest
            .issues
            .iter()
            .any(|issue| issue.kind == "material_library"),
        "authored library failure remains visible"
    );
    let bytes = std::fs::read(mesh_file(&written.out))?;
    let checked = check_glb(&bytes)?;
    assert!(
        checked
            .document
            .materials()
            .any(|material| material.name() == Some("archive-fallback")),
        "the actual rigged GLB uses the physical fallback"
    );
    Ok(())
}
