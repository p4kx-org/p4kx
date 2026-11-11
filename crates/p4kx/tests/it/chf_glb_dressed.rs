// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx chf glb` of a character that wears eyes and hair, over a synthetic Archive.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use p4kx_assembly::character::{BODY_COLOR, Guid, lowered_hash, skin_attachment};
use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{
    DRESSED_EYES, DRESSED_GONE_HAIR, DRESSED_HAIR, DRESSED_HAIR_OPACITY, DRESSED_HEAD_DIFFUSE,
    DRESSED_HEAD_MATERIAL, DRESSED_HEAD_MATERIAL_FILE, HEAD_REGIONS, HEAD_VERTICES, MALE_BODY_TYPE,
    MALE_RIG_HASH, SplitDds, TestChf, TestEntry, TestItemPort, TestMaterial, TestSubMaterial,
    TestTextureFormat, check_glb, dressed_entries, head_rig, test_key,
};

use serde_json::Value;

use crate::chf_glb::{archive, glb_with};

type TestResult = Result<(), Box<dyn Error>>;

#[path = "chf_glb_selectors.rs"]
mod selectors;

fn stored(guid: u128) -> Result<[u8; 16], Box<dyn Error>> {
    let text = format!("{guid:032x}");
    let dashed = format!(
        "{}-{}-{}-{}-{}",
        &text[..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..]
    );
    Ok(dashed.parse::<Guid>()?.stored())
}

fn item(
    name: &str,
    guid: u128,
    children: Vec<TestItemPort>,
) -> Result<TestItemPort, Box<dyn Error>> {
    Ok(TestItemPort {
        port: lowered_hash(name),
        item: stored(guid)?,
        tail_count: 0,
        children,
    })
}

fn dressed_chf(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let head = item(
        "head_itemport",
        0x900,
        vec![
            item("hair_itemport", DRESSED_HAIR, Vec::new())?,
            item("eyes_itemport", DRESSED_EYES, Vec::new())?,
            item("beard_itemport", DRESSED_GONE_HAIR, Vec::new())?,
        ],
    )?;
    let test = TestChf {
        body_type: MALE_BODY_TYPE,
        dna_hashes: Some((MALE_RIG_HASH, 0)),
        face_parts: (0..HEAD_REGIONS).map(|_| vec![(0, 0), (0, 0)]).collect(),
        ports: Some(item("body_itemport", 0x901, vec![head])?),
        materials: vec![TestMaterial {
            attachment: skin_attachment("head_itemport"),
            base_material: stored(DRESSED_HEAD_MATERIAL)?,
            flags: 0,
            sub_materials: vec![TestSubMaterial {
                name: lowered_hash("shader_head"),
                textures: Vec::new(),
                floats: Vec::new(),
                colors: vec![(BODY_COLOR, [255, 0, 0, 255])],
            }],
        }],
        ..TestChf::v8()
    };
    let path = dir.join("dressed.chf");
    fs::write(&path, test.to_bytes())?;
    Ok(path)
}

fn textured_entries() -> Vec<TestEntry> {
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let mut entries = dressed_entries(Some(rig));
    let texture = SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4).mips(1);
    for file in [DRESSED_HEAD_DIFFUSE, DRESSED_HAIR_OPACITY] {
        let dds = file.replace('/', "\\").replace(".tif", ".dds");
        entries.extend(texture.entries(&format!("Data\\{dds}")));
    }
    entries
}

#[test]
fn a_dressed_character_is_written_with_eyes_hair_textures_and_a_report_of_what_is_missing()
-> TestResult {
    let dir = tempfile::tempdir()?;
    let entries = textured_entries();
    let head_material = format!("Data\\{}", DRESSED_HEAD_MATERIAL_FILE.replace('/', "\\"));
    let entries = entries
        .into_iter()
        .map(|entry| {
            if entry.path().eq_ignore_ascii_case(&head_material) {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect();
    let archive = archive(dir.path(), entries)?;
    let file = dressed_chf(dir.path())?;
    let out = dir.path().join("dressed.glb");

    let key = test_key().to_hex();
    let run = glb_with(&archive, &file, &out, true, &["--key", &key])?;

    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let value: Value = serde_json::from_str(&run.stdout)?;
    let names: Vec<&str> = value["parts"]
        .as_array()
        .ok_or("parts")?
        .iter()
        .filter_map(|part| part["name"].as_str())
        .collect();
    assert_eq!(
        names,
        ["body", "head", "teeth", "hair_itemport", "eyes_itemport"]
    );
    assert_eq!(value["skipped"], 1, "the beard's skin is missing");
    let kinds: Vec<&str> = value["issues"]
        .as_array()
        .ok_or("issues")?
        .iter()
        .filter_map(|issue| issue["kind"].as_str())
        .collect();
    assert!(kinds.contains(&"geometry_missing"), "{kinds:?}");
    assert!(value["textures"].as_u64().unwrap_or(0) >= 2, "{value}");
    assert!(
        value["textured_materials"].as_u64().unwrap_or(0) >= 2,
        "{value}"
    );
    check_image_origins(&out, &value)
}

fn check_image_origins(out: &Path, value: &Value) -> TestResult {
    let bytes = fs::read(out)?;
    let checked = check_glb(&bytes)?;
    let receipt = image_receipt(out, &bytes, value)?;
    for (name, encrypted) in [("shader_head", true), ("hair_test_m", false)] {
        let image = checked
            .document
            .materials()
            .find(|material| material.name() == Some(name))
            .and_then(|material| material.pbr_metallic_roughness().base_color_texture())
            .ok_or("material base colour image")?
            .texture()
            .source();
        let image_index = image.index();
        let p4kx_testkit::gltf::image::Source::View { view, .. } = image.source() else {
            return Err("embedded image buffer view".into());
        };
        let path = format!("/bufferViews/{}", view.index());
        let row = receipt.get(&path).ok_or("PNG range in GLB receipt")?;
        assert_eq!(
            row.get("containsEncryptedSource").and_then(Value::as_bool),
            Some(encrypted),
            "the physical PNG has the selected image source of {name}"
        );
        let (start, end) = range(row)?;
        assert!(
            bytes
                .get(start..end)
                .is_some_and(|data| data.starts_with(b"\x89PNG\r\n\x1a\n")),
            "the absolute receipt range addresses PNG bytes for {name}"
        );
        let field = value
            .pointer(&format!(
                "/textureImageOrigins/{image_index}/containsEncryptedSource"
            ))
            .and_then(Value::as_bool);
        assert_eq!(
            field,
            Some(encrypted),
            "the selected image source of {name}"
        );
    }
    Ok(())
}

fn image_receipt(
    out: &Path,
    bytes: &[u8],
    report: &Value,
) -> Result<BTreeMap<String, Value>, Box<dyn Error>> {
    let name = report
        .get("originReceipt")
        .and_then(Value::as_str)
        .ok_or("receipt path")?;
    let data = fs::read_to_string(out.with_file_name(name))?;
    let glb = p4kx_testkit::gltf::binary::Glb::from_slice(bytes)?;
    let document: Value = serde_json::from_slice(&glb.json)?;
    let bin_length = glb.bin.as_ref().ok_or("binary chunk")?.len();
    let bin_start = bytes
        .len()
        .checked_sub(bin_length)
        .ok_or("binary chunk start")?;
    let mut ranges = BTreeMap::new();
    let mut images = 0;
    let mut rows = data.lines().map(serde_json::from_str::<Value>);
    check_binding(&rows.next().ok_or("receipt header")??, bytes)?;
    for row in rows {
        let row = row?;
        let Some(kind) = row.pointer("/address/type").and_then(Value::as_str) else {
            continue;
        };
        match kind {
            "glbJsonField" => {
                let path = address_path(&row)?;
                assert!(
                    document.pointer(path).is_some(),
                    "actual GLB JSON field {path}"
                );
                if path.starts_with("/images/") {
                    images += 1;
                }
            }
            "glbBinaryBytes" => {
                let path = address_path(&row)?.to_owned();
                check_view(&document, &path, &row, bin_start, bytes.len())?;
                assert!(
                    ranges.insert(path, row).is_none(),
                    "one receipt row per buffer view"
                );
            }
            _ => {}
        }
    }
    assert_eq!(
        ranges.len(),
        array_len(&document, "bufferViews")?,
        "all written buffer views have exact ranges"
    );
    assert_eq!(
        images,
        array_len(&document, "images")?,
        "all written images have JSON addresses"
    );
    Ok(ranges)
}

fn array_len(document: &Value, name: &str) -> Result<usize, Box<dyn Error>> {
    document
        .get(name)
        .and_then(Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| format!("missing GLB array {name}").into())
}

fn check_view(
    document: &Value,
    path: &str,
    row: &Value,
    bin_start: usize,
    file_length: usize,
) -> TestResult {
    let view = document.pointer(path).ok_or("actual GLB buffer view")?;
    let offset = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0);
    let length = view
        .get("byteLength")
        .and_then(Value::as_u64)
        .ok_or("buffer view length")?;
    let (start, end) = range(row)?;
    assert_eq!(
        start,
        bin_start + usize::try_from(offset)?,
        "start of {path}"
    );
    assert_eq!(end, start + usize::try_from(length)?, "end of {path}");
    assert!(end <= file_length, "view remains within the measured GLB");
    Ok(())
}

fn check_binding(header: &Value, bytes: &[u8]) -> TestResult {
    let mut actual = OriginDigestWriter::new(io::sink());
    actual.write_all(bytes)?;
    let actual = serde_json::to_value(actual.finish()?)?;
    for name in ["byteLength", "sha256"] {
        assert_eq!(
            header.pointer(&format!("/binding/{name}")),
            Some(actual.get(name).ok_or("measured digest field")?),
            "the address is bound to the actual artifact {name}"
        );
    }
    Ok(())
}

fn address_path(row: &Value) -> Result<&str, Box<dyn Error>> {
    row.pointer("/address/path")
        .and_then(Value::as_str)
        .ok_or_else(|| "GLB address pointer".into())
}

fn range(row: &Value) -> Result<(usize, usize), Box<dyn Error>> {
    let bound = |name| -> Result<usize, Box<dyn Error>> {
        Ok(row
            .pointer(name)
            .and_then(Value::as_str)
            .ok_or("decimal byte coordinate")?
            .parse()?)
    };
    Ok((bound("/address/start")?, bound("/address/endExclusive")?))
}

#[test]
fn the_colours_mode_writes_no_texture() -> TestResult {
    let dir = tempfile::tempdir()?;
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let archive = archive(dir.path(), dressed_entries(Some(rig)))?;
    let file = dressed_chf(dir.path())?;
    let out = dir.path().join("plain.glb");

    let output = p4kx_testkit::p4kx()?
        .args(["--json", "chf", "glb"])
        .arg(&archive)
        .arg(&file)
        .arg("--out")
        .arg(&out)
        .args(["--materials", "colours"])
        .output()?;

    assert_eq!(
        output.status.code(),
        Some(0),
        "a missing skin is listed, not fatal"
    );
    let value: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(value["textures"], 0);
    assert_eq!(value["textured_materials"], 0);
    Ok(())
}
