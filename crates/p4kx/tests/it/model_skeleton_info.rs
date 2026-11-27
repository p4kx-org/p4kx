// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx model info` on bone names, levels with a skeleton of their own and bad texture
//! coordinates.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::{ArchiveBuilder, TestBone, TestChunkFile, TestEntry, TestModel, TestSkeleton};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const SKIN_ENTRY: &str = "Data\\Objects\\Test\\skin.skin";
const SKELETON_ENTRY: &str = "Data/Objects/Test/skeleton.chr";
const COMPILED_BONES: u32 = 0xC201_973C;

fn skeleton() -> TestSkeleton {
    TestSkeleton::new(vec![
        TestBone {
            name: "bad\u{1b}[2Jname".to_owned(),
            parent: None,
            rotation: [0.0, 0.0, 0.0, 1.0],
            translation: [0.0; 3],
        },
        TestBone {
            name: "tip".to_owned(),
            parent: Some(0),
            rotation: [0.0, 0.0, 0.0, 1.0],
            translation: [0.0, 0.0, 1.0],
        },
    ])
}

fn skin() -> TestModel {
    let level_one = TestModel::cube()
        .with_skeleton(&TestSkeleton::chain(6))
        .with_bone_map(false, vec![vec![(5, 255)]; 24]);
    TestModel::cube()
        .with_skeleton(&skeleton())
        .with_bone_map(false, vec![vec![(1, 255)]; 24])
        .with_uv(0, [f32::NAN, f32::INFINITY])
        .with_lod(level_one)
}

fn write_archive(folder: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = folder.join("Data.p4k");
    let skin = skin();
    let folder = "Data\\Objects\\Test\\";
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(SKIN_ENTRY, skin.cgf()))
        .entry(TestEntry::zstd(format!("{folder}skin.skinm"), skin.cgfm()))
        .entry(
            TestEntry::zstd(
                format!("{folder}skin_lod1.skin"),
                skin.lod_cgf(1).ok_or("level 1 file")?,
            )
            .encrypted(),
        )
        .entry(TestEntry::zstd(
            format!("{folder}skin_lod1.skinm"),
            skin.lod_cgfm(1).ok_or("level 1 geometry")?,
        ))
        .write_to(&path)?;
    Ok(path)
}

fn info(args: &[&str]) -> Result<Vec<u8>, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    read_info(&archive, Some(SKIN_ENTRY), args)
}

fn read_info(path: &Path, entry: Option<&str>, args: &[&str]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command
        .args(args)
        .args(["--key", &p4kx_testkit::test_key().to_hex(), "model", "info"])
        .arg(path);
    if let Some(entry) = entry {
        command.arg(entry);
    }
    let output = command.output()?;
    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    Ok(output.stdout)
}

#[test]
fn bone_names_are_sanitised_before_they_reach_the_terminal() -> TestResult {
    let text = String::from_utf8(info(&[])?)?;

    assert!(!text.contains('\u{1b}'), "no escape character: {text:?}");
    assert!(text.contains("bad\u{fffd}[2Jname"), "replaced: {text:?}");
    Ok(())
}

#[test]
fn a_level_with_its_own_skeleton_shows_its_bone_count() -> TestResult {
    let text = String::from_utf8(info(&[])?)?;
    let levels: Vec<&str> = text
        .lines()
        .filter(|line| line.contains("Skeleton") || line.trim_end().ends_with("bones"))
        .collect();
    assert_eq!(
        levels.len(),
        2,
        "header and the level with a skeleton: {text}"
    );
    assert!(levels[1].contains("6 bones"), "{}", levels[1]);

    let json: Value = serde_json::from_slice(&info(&["--json"])?)?;
    assert_eq!(json["lods"][0]["skeleton_bones"], Value::Null, "level 0");
    assert_eq!(json["lods"][1]["skeleton_bones"], 6, "level 1");
    Ok(())
}

#[test]
fn bad_texture_coordinates_are_counted_per_mesh() -> TestResult {
    let json: Value = serde_json::from_slice(&info(&["--json"])?)?;

    assert_eq!(json["lods"][0]["meshes"][0]["non_finite_uvs"], 1, "level 0");
    assert_eq!(json["lods"][1]["meshes"][0]["non_finite_uvs"], 0, "level 1");
    let text = String::from_utf8(info(&[])?)?;
    assert!(text.contains("Bad UVs"), "{text}");
    Ok(())
}

#[test]
fn model_json_preserves_primary_bone_origins_with_an_encrypted_lod() -> TestResult {
    let json: Value = serde_json::from_slice(&info(&["--json"])?)?;
    assert_eq!(
        json.pointer("/bones/1/name"),
        Some(&json!("tip")),
        "primary bone"
    );
    for path in [
        "/bones",
        "/bones/1/index",
        "/bones/1/name",
        "/bones/1/world/translation/0",
    ] {
        assert_primary_bone(&json, path)?;
    }
    let chunks = json
        .get("chunks")
        .and_then(Value::as_array)
        .ok_or("chunk rows")?;
    let lod = chunks
        .iter()
        .position(|chunk| chunk.get("file").and_then(Value::as_str) == Some("lod1-model"))
        .ok_or("the independent LOD model must supply a chunk")?;
    let lod_origin = bone_origin(&json, &format!("/chunks/{lod}/type_id"))?;
    assert_eq!(
        lod_origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "the encrypted LOD remains an independent source"
    );
    Ok(())
}

fn assert_primary_bone(info: &Value, path: &str) -> TestResult {
    let origin = bone_origin(info, path)?;
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(false),
        "encryption follows the primary skeleton for {path}"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "primary skeleton coverage for {path}"
    );
    let uses = bone_uses(info, origin)?;
    let sources = info
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("missing physical source inventory")?;
    assert!(!uses.is_empty(), "physical contributors for {path}");
    let primary = SKIN_ENTRY.replace('\\', "/");
    for used in uses {
        let owner = sources
            .iter()
            .find(|source| source.get("id") == used.get("source"))
            .and_then(|source| source.pointer("/path/value"))
            .and_then(Value::as_str)
            .ok_or("missing primary Entry path")?;
        assert_eq!(owner.replace('\\', "/"), primary, "owner of {path}");
    }
    Ok(())
}

#[test]
fn model_json_binds_the_compact_bone_words_to_decoded_primary_offsets() -> TestResult {
    let file = TestChunkFile::ivo()
        .chunk(
            COMPILED_BONES,
            0,
            TestSkeleton::two_bones().compiled_bones(),
        )
        .table_at_end();
    let start = *file.offsets().first().ok_or("skeleton chunk offset")?;
    let bytes = file.bytes();
    let table_offset = bytes.get(12..16).ok_or("skeleton table offset word")?;
    let table_offset = u64::from(u32::from_le_bytes(table_offset.try_into()?));
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(SKELETON_ENTRY, bytes).encrypted())
        .write_to(&archive)?;
    let json: Value =
        serde_json::from_slice(&read_info(&archive, Some(SKELETON_ENTRY), &["--json"])?)?;
    assert_eq!(
        json.pointer("/bones/1/name"),
        Some(&json!("tip")),
        "authored tip"
    );
    assert_eq!(
        json.pointer("/bones/1/node"),
        Some(&json!(1)),
        "compact node word"
    );
    let collection = bone_origin(&json, "/bones")?;
    assert_eq!(
        collection
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "collection uses the encrypted primary Entry"
    );
    assert_compact_bone_sites(&json, start, table_offset)
}

fn assert_compact_bone_sites(json: &Value, start: u64, table_offset: u64) -> TestResult {
    assert_site(
        json,
        "/bones",
        "cryEngineChunkTable",
        "skeletonTypePrefix",
        "selector",
        &[table_offset, table_offset + 4, 16, 1],
    )?;
    for (path, field, role, res) in [
        ("node", "node", "conversion", 78..80),
        ("parent", "parent", "conversion", 72..74),
        (
            "world/translation/1",
            "world.translation",
            "payload",
            193..197,
        ),
        (
            "relative/rotation/0",
            "relative.rotation",
            "payload",
            117..121,
        ),
    ] {
        assert_site(
            json,
            &format!("/bones/1/{path}"),
            "cryEngineSkeleton",
            field,
            role,
            &[0, 1, start + res.start, start + res.end],
        )?;
    }
    Ok(())
}

#[test]
fn model_json_marks_loose_bones_and_their_collection_unknown() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("skeleton.chr");
    let file = TestChunkFile::ivo().chunk(
        COMPILED_BONES,
        0,
        TestSkeleton::two_bones().compiled_bones(),
    );
    fs::write(&path, file.bytes())?;
    let json: Value = serde_json::from_slice(&read_info(&path, None, &["--json"])?)?;
    let unknown = json!({
        "containsEncryptedSource": null,
        "sourceSetComplete": false,
        "sourceUsesRefs": []
    });
    for path in ["/bones", "/bones/1/name", "/bones/1/world/rotation/3"] {
        assert!(json.pointer(path).is_some(), "emitted value at {path}");
        assert_eq!(bone_origin(&json, path)?, &unknown, "unbound {path}");
    }
    Ok(())
}

fn bone_origin<'a>(info: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    info.get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or_else(|| format!("missing field origin at {path}").into())
}

fn bone_uses<'a>(info: &'a Value, origin: &Value) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let references = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("bone references")?;
    let inventory = info
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let uses: Vec<_> = inventory
        .iter()
        .filter(|used| used.get("id").is_some_and(|id| references.contains(id)))
        .collect();
    assert_eq!(
        uses.len(),
        references.len(),
        "all bone contributors resolve"
    );
    Ok(uses)
}

fn assert_site(
    info: &Value,
    path: &str,
    namespace: &str,
    field: &str,
    role: &str,
    indices: &[u64],
) -> TestResult {
    assert!(info.pointer(path).is_some(), "emitted value at {path}");
    let origin = bone_origin(info, path)?;
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "complete site {path}"
    );
    let indices = json!(indices.iter().map(u64::to_string).collect::<Vec<_>>());
    assert!(
        bone_uses(info, origin)?.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some(role)
                && used.get("scope").and_then(Value::as_str) == Some("payload")
                && used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && used.pointer("/location/indices") == Some(&indices)
        }),
        "the {role} site for {field} at {path} has coordinates {indices}"
    );
    Ok(())
}
