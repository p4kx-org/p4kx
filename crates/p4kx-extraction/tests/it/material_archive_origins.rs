// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::model::LayerCache;
use p4kx_extraction::{
    Archive, ArchiveMaterialReads, FieldOrigins, FieldPath, ModelSource, OpenOptions, PathIndex,
    SourceEncryption, SourceUses, TextureVerdicts,
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

    check_shared_layer(&index, &cache, &verdicts, &reads, &warmed)?;

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

fn check_shared_layer(
    index: &PathIndex<'_>,
    cache: &LayerCache,
    verdicts: &TextureVerdicts,
    reads: &ArchiveMaterialReads,
    warmed: &std::sync::Arc<p4kx_extraction::model::MaterialStatus>,
) -> TestResult {
    let (cached, original) = ModelSource::layer_library_observed(
        index,
        "LAYERS/SHARED.MTL",
        cache,
        verdicts,
        &mut cache.budget(),
        reads,
    );
    assert!(
        std::sync::Arc::ptr_eq(&cached, warmed),
        "observed lookup reuses the already cached decode"
    );
    let original = original.ok_or("the exact cached layer Entry")?;
    assert_eq!(
        original.entry().path(),
        r"Data\layers\shared.mtl",
        "physical path is from the actual cached read"
    );
    assert_eq!(
        original.encryption(),
        SourceEncryption::Unencrypted,
        "a plain cached layer retains its own encryption state"
    );
    Ok(())
}

#[test]
fn model_material_summary_retains_only_the_actual_loaded_library_and_its_selector() -> TestResult {
    use p4kx_testkit::TestModel;

    const MODEL: &str = "Data/objects/test/cube.cgf";
    const LIBRARY: &str = "Data/objects/test/cube.mtl";
    let fixture = TestModel::cube();
    let material =
        TestMtl::single(&TestMtlMaterial::new("selected")).with_default_palette("opaque");
    let directory = tempfile::tempdir()?;
    let library_path = FieldPath::parse("/structuredContent/library")?;
    let count_path = FieldPath::parse("/structuredContent/counts/materials")?;
    for loaded in [true, false] {
        let archive_path = directory
            .path()
            .join(if loaded { "loaded.p4k" } else { "invalid.p4k" });
        let bytes = if loaded {
            material.cryxml()
        } else {
            b"invalid".to_vec()
        };
        ArchiveBuilder::new()
            .entry(TestEntry::stored(MODEL, fixture.cgf()))
            .entry(TestEntry::stored(
                "Data/objects/test/cube.cgfm",
                fixture.cgfm(),
            ))
            .entry(TestEntry::stored(LIBRARY, bytes).encrypted())
            .write_to(&archive_path)?;
        let archive = Archive::open_with(archive_path, OpenOptions::new().key(Some(test_key())))?;
        let model = ModelSource::from_index_with_material_summary_origins(
            &PathIndex::new(&archive),
            MODEL,
            &TextureVerdicts::new(),
        )?;
        let uses = SourceUses::with_compact_use_refs();
        let origins = model.loaded_material_summary_origins(&uses, &library_path, &count_path)?;
        assert_eq!(model.model().materials.library().is_some(), loaded);
        if !loaded {
            assert_eq!(
                origins.iter().count(),
                0,
                "the failed payload contributes no loaded fields"
            );
            assert_eq!(
                uses.freeze()?.source_count(),
                0,
                "the failed source is not payload evidence"
            );
            continue;
        }
        check_loaded_summary(&origins, &uses)?;
    }
    Ok(())
}

fn check_loaded_summary(origins: &FieldOrigins, source_uses: &SourceUses) -> TestResult {
    for pointer in [
        "/structuredContent/library/single",
        "/structuredContent/library/default_palette",
        "/structuredContent/library/authored/encoding",
        "/structuredContent/counts/materials",
    ] {
        let origin = origins
            .get(&FieldPath::parse(pointer)?)
            .ok_or_else(|| format!("missing exact origin at {pointer}"))?;
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "original loaded bytes at {pointer}"
        );
        assert!(
            !origin.source_set_complete(),
            "resolver decisions are not part of this payload observation"
        );
    }
    let inventory = serde_json::to_value(source_uses.freeze()?)?;
    let uses = inventory
        .get("uses")
        .and_then(serde_json::Value::as_array)
        .ok_or("source uses")?;
    assert!(
        uses.iter().any(|entry| {
            entry
                .pointer("/location/field")
                .and_then(serde_json::Value::as_str)
                == Some("materialLibrary")
                && entry.get("role").and_then(serde_json::Value::as_str) == Some("selector")
        }),
        "the original selected model declaration"
    );
    assert!(
        uses.iter().any(|entry| {
            entry
                .pointer("/location/field")
                .and_then(serde_json::Value::as_str)
                == Some("DefaultPalette")
        }),
        "the original selected library attribute"
    );
    Ok(())
}
