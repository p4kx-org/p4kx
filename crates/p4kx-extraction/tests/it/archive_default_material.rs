// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::sync::Arc;

use p4kx_extraction::model::MaterialStatus;
use p4kx_extraction::{
    AnimationSet, ArchiveDefaultMaterialState, GlbOptions, ModelInfo, ModelSource, PathIndex,
    SourceEncryption, TextureVerdicts,
};
use p4kx_testkit::{MATERIAL_REFERENCE_MODEL, TestEntry, check_glb, test_key};
use serde_json::json;
use sha2::{Digest as _, Sha256};

use super::material_reference_support::{TARGET, TestResult, archive as fixture};

const DEFAULT_ENTRY: &str = "Data/Materials/material_default.mtl";
const PHYSICAL: &[u8] =
    br#"<Material Name="archive-fallback" Shader="Illum" Diffuse="0.1,0.2,0.3"/>"#;
const AUTHORED: &str = "/Objects/Test/paint.mtl";

#[test]
fn original_archive_fallback_drives_glb_and_retains_authored_lookup_failure() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = fixture(
        dir.path(),
        AUTHORED,
        None,
        [TestEntry::stored(DEFAULT_ENTRY, PHYSICAL)],
    )?;
    let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
    assert!(
        matches!(source.model().materials, MaterialStatus::Missing { .. }),
        "authored failure stays independent"
    );
    let read = source.default_material().ok_or("model fallback evidence")?;
    assert!(
        matches!(read.state(), ArchiveDefaultMaterialState::Resolved),
        "verified fallback"
    );
    assert_eq!(
        read.decoded_sha256().map(<[u8; 32]>::as_slice),
        Some(Sha256::digest(PHYSICAL).as_slice()),
        "verified decoded fallback digest"
    );
    let original = read.original().ok_or("physical fallback source")?;
    assert!(
        archive.owns_origin(original),
        "exact opened source owns the original Entry"
    );
    assert_eq!(
        original.entry().path(),
        DEFAULT_ENTRY,
        "fallback path is physical"
    );
    let metadata = serde_json::to_value(ModelInfo::of(&source, &AnimationSet::default()))?;
    assert_eq!(
        metadata.pointer("/default_material/state"),
        Some(&json!("resolved")),
        "typed lookup result"
    );
    assert_eq!(
        metadata.pointer("/default_material/source/contains_encrypted_source"),
        Some(&json!(false)),
        "source marker"
    );
    check_fallback_glb(&source, &dir.path().join("physical.glb"))
}

fn check_fallback_glb(source: &ModelSource, path: &std::path::Path) -> TestResult {
    let summary = source.write_glb(&GlbOptions::default(), path)?;
    let bytes = fs::read(path)?;
    let glb = check_glb(&bytes)?;
    assert_eq!(
        summary.materials, 1,
        "one dense GLB material for shared fallback"
    );
    assert_eq!(
        summary.default_material_ids, 2,
        "both original material IDs select fallback"
    );
    let mesh = glb.document.meshes().next().ok_or("synthetic model mesh")?;
    assert!(
        mesh.primitives()
            .all(|primitive| primitive.material().name() == Some("archive-fallback")),
        "GLB uses the source material"
    );
    Ok(())
}

#[test]
fn fallback_cache_follows_archive_key_without_replacing_authored_libraries() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = fixture(
        dir.path(),
        AUTHORED,
        Some(TARGET),
        [TestEntry::zstd(DEFAULT_ENTRY, PHYSICAL).encrypted()],
    )?;
    let index = PathIndex::new(&archive);
    let verdicts = TextureVerdicts::new();
    let before = ModelSource::archive_default_material(&index, &verdicts);
    assert!(
        matches!(before.state(), ArchiveDefaultMaterialState::Unreadable(_)),
        "no key leaves fallback unread"
    );
    assert_eq!(
        before
            .original()
            .map(p4kx_extraction::EntryOrigin::encryption),
        Some(SourceEncryption::Encrypted),
        "actual attempted Entry keeps encryption evidence"
    );
    archive.set_key(test_key());
    let after = ModelSource::archive_default_material(&index, &verdicts);
    assert!(
        matches!(after.state(), ArchiveDefaultMaterialState::Resolved),
        "key update invalidates failed cache state"
    );
    assert!(
        !Arc::ptr_eq(&before, &after),
        "different key generation gets a new read"
    );
    assert!(
        Arc::ptr_eq(
            &after,
            &ModelSource::archive_default_material(&index, &verdicts)
        ),
        "same Archive and key share one observed read"
    );
    archive.clear_key();
    let normal = ModelSource::from_index(&index, MATERIAL_REFERENCE_MODEL)?;
    assert!(
        matches!(normal.model().materials, MaterialStatus::Loaded(_)),
        "fallback failure cannot poison unrelated authored library"
    );
    let cleared = ModelSource::archive_default_material(&index, &verdicts);
    assert!(
        matches!(cleared.state(), ArchiveDefaultMaterialState::Unreadable(_)),
        "cleared key cannot reuse decrypted fallback"
    );
    Ok(())
}

#[test]
fn missing_malformed_ambiguous_and_unsupported_sources_remain_distinct() -> TestResult {
    let dir = tempfile::tempdir()?;
    for (entries, expected) in [
        (vec![], "missing"),
        (
            vec![TestEntry::stored(DEFAULT_ENTRY, b"<Material")],
            "malformed",
        ),
        (
            vec![
                TestEntry::stored(DEFAULT_ENTRY, PHYSICAL),
                TestEntry::stored("data/materials/MATERIAL_DEFAULT.MTL", PHYSICAL),
            ],
            "ambiguous",
        ),
        (
            vec![TestEntry::stored(
                DEFAULT_ENTRY,
                b"<Material><SubMaterials><Material/></SubMaterials></Material>",
            )],
            "unsupported-library",
        ),
    ] {
        let archive = fixture(dir.path(), AUTHORED, None, entries)?;
        let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
        let fallback = source.default_material().ok_or("fallback decision")?;
        assert!(
            fallback.material().is_none(),
            "unusable physical fallback leaves synthetic policy"
        );
        let value = serde_json::to_value(ModelInfo::of(&source, &AnimationSet::default()))?;
        assert_eq!(
            value.pointer("/default_material/state"),
            Some(&json!(expected)),
            "fallback cause is separate"
        );
        if let ArchiveDefaultMaterialState::Ambiguous { witnesses } = fallback.state() {
            assert_ne!(
                witnesses[0].entry().index(),
                witnesses[1].entry().index(),
                "two original physical conflict witnesses"
            );
        }
    }
    Ok(())
}

#[test]
fn one_authored_material_broadcasts_and_empty_multi_slot_uses_physical_fallback() -> TestResult {
    let dir = tempfile::tempdir()?;
    for (library, expected, fallback_count) in [
        (br#"<Material Name="single-authored" Shader="Illum"/>"#.as_slice(), vec!["single-authored"], 0),
        (br#"<Material><SubMaterials><Material/><Empty/><Material Name="side" Shader="Illum"/></SubMaterials></Material>"#.as_slice(), vec!["archive-fallback", "side"], 1),
        (br#"<Material><SubMaterials><Material/><Material Name="hidden" Shader="NoDraw"/></SubMaterials></Material>"#.as_slice(), vec!["archive-fallback"], 1),
    ] {
        let archive = fixture(dir.path(), AUTHORED, None, [
            TestEntry::stored(DEFAULT_ENTRY, PHYSICAL),
            TestEntry::stored(TARGET, library),
        ])?;
        let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
        let library = source.model().materials.library().ok_or("authored library remains loaded")?;
        if expected.contains(&"side") {
            assert!(library.is_empty_slot(1), "authored sparse position was retained");
        }
        let path = dir.path().join("slots.glb");
        let summary = source.write_glb(&GlbOptions::default(), &path)?;
        assert_eq!(summary.default_material_ids, fallback_count, "only actual fallback IDs are counted");
        let bytes = fs::read(path)?;
        let document = check_glb(&bytes)?.document;
        let names = document.materials().filter_map(|material| material.name().map(str::to_owned)).collect::<Vec<_>>();
        assert_eq!(names, expected, "dense materials retain effective authored and fallback choices");
    }
    Ok(())
}

#[test]
fn physical_default_keeps_the_selected_layer_entry_and_slot() -> TestResult {
    const LAYER: &str = "Data/Materials/Test/fallback_layer.mtl";
    let folder = tempfile::tempdir()?;
    let default = br#"<Material Name="physical" Shader="Illum"><MatLayers><Layer Path="Materials/Test/fallback_layer" Submtl="chosen"/></MatLayers></Material>"#;
    let layer = br#"<Material><SubMaterials><Material Name="unused"/><Material Name="chosen"/></SubMaterials></Material>"#;
    let archive = fixture(
        folder.path(),
        AUTHORED,
        None,
        [
            TestEntry::stored(DEFAULT_ENTRY, default),
            TestEntry::zstd(LAYER, layer).encrypted(),
        ],
    )?;
    archive.set_key(test_key());
    let source = ModelSource::from_archive(&archive, MATERIAL_REFERENCE_MODEL)?;
    let fallback = source.default_material().ok_or("fallback observation")?;
    assert!(fallback.material().is_some(), "verified root is drawable");
    let layer = fallback
        .material_origins()
        .layer(0, 0)
        .ok_or("selected layer origin")?;
    assert_eq!(layer.entry().entry().path(), LAYER, "physical layer Entry");
    assert_eq!(
        layer.entry().encryption(),
        SourceEncryption::Encrypted,
        "original layer marker"
    );
    assert_eq!(
        layer.source_material(),
        Some(1),
        "the chosen authored layer slot"
    );
    Ok(())
}
