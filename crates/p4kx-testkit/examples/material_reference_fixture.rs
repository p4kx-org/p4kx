// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Writes four synthetic Archives for material-reference consumer verification.

use std::error::Error;
use std::path::PathBuf;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, entity_schema,
    material_identity_entries, material_reference_entries,
};

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("provide an absolute new fixture directory")?;
    if !output.is_absolute() || output.exists() {
        return Err("fixture directory must be absolute and new".into());
    }
    let entity = TestEntity::new(1, "TEST_MaterialReference", "entities/test_material.xml")
        .geometry("Objects/Test/cube.cgf", None);
    let dataforge = entity_schema(DataForgeBuilder::new())
        .record(entity.record())
        .build();
    let single = |reference| {
        material_reference_entries(reference, Some("Data/Objects/Test/paint.mtl"))
            .into_iter()
            .chain([TestEntry::stored("Data/Game2.dcb", dataforge.clone())])
            .collect::<Vec<_>>()
    };
    std::fs::create_dir_all(&output)?;
    for (case, entries) in [
        ("virtual-root", single("/Objects/Test/paint.mtl")),
        (
            "authoring",
            single("//authoring-host/branch/Data/Objects/Test/paint.mtl"),
        ),
        ("collision-virtual", material_identity_entries("/paint.mtl")),
        (
            "collision-authoring",
            material_identity_entries("//host/branch/Data/paint.mtl"),
        ),
    ] {
        entries
            .into_iter()
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(output.join(format!("{case}.p4k")))?;
    }
    Ok(())
}
