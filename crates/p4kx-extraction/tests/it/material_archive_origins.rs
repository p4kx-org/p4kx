// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::model::LayerCache;
use p4kx_extraction::{
    Archive, ArchiveMaterialReads, ModelSource, OpenOptions, PathIndex, SourceEncryption,
    TextureVerdicts,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestMtl, TestMtlMaterial, test_key};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn material_sources_follow_the_actual_sibling_and_already_cached_layer_selection() -> TestResult {
    const MAIN: &str = r"Data\Objects\Test\main.mtl";
    const LAYER: &str = r"Data\layers\shared.mtl";
    let main = TestMtl::single(
        &TestMtlMaterial::new("main")
            .layer_with(&[("Path", "layers/shared"), ("Submtl", "chosen")])
            .layer_with(&[("Path", "layers/shared"), ("Submtl", "missing")]),
    );
    let layer = TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        TestMtlMaterial::new("chosen"),
    ]);
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("materials.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(MAIN, main.cryxml()).encrypted())
        .entry(TestEntry::stored(r"Data\main.mtl", main.cryxml()))
        .entry(TestEntry::stored(LAYER, layer.cryxml()))
        .write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let index = PathIndex::new(&archive);
    let cache = LayerCache::new();
    let verdicts = TextureVerdicts::new();
    let reads = ArchiveMaterialReads::default();
    let warmed = ModelSource::layer_library_tracking(
        &index,
        "layers/shared",
        &cache,
        &verdicts,
        &mut cache.budget(),
        Some(&reads),
    );
    assert!(warmed.library().is_some());

    let read = ModelSource::material_library_observed(
        &index,
        r"Data\Objects\Test\test.cgf",
        "main",
        &cache,
        &verdicts,
        &reads,
    );
    assert!(read.status.library().is_some());
    let main = read.origins.library().ok_or("selected library origin")?;
    assert_eq!(main.entry().path(), MAIN);
    assert_eq!(main.encryption(), SourceEncryption::Encrypted);
    let selected = read.origins.layer(0, 0).ok_or("selected layer origin")?;
    assert_eq!(selected.entry().entry().path(), LAYER);
    assert_eq!(selected.entry().encryption(), SourceEncryption::Unencrypted);
    assert_eq!(selected.source_material(), Some(1));
    let attempted = read.origins.layer(0, 1).ok_or("attempted layer origin")?;
    assert_eq!(attempted.entry().entry().path(), LAYER);
    assert_eq!(attempted.source_material(), None);
    assert!(read.origins.layer(0, 2).is_none());
    Ok(())
}
