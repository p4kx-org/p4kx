// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_testkit::{
    HEAD_VERTICES, MALE_RIG_HASH, SplitDds, TestEntry, TestMtl, TestMtlMaterial, TestTextureFormat,
    check_glb, head_entries, head_rig, test_key,
};
use serde_json::Value;

use super::{TestResult, archive, chf, glb_with};

const FALLBACK_NAME: &str = "archive-fallback";
const DEFAULT: &str = "Data/Materials/material_default.mtl";

fn entries() -> Vec<TestEntry> {
    let rig = head_rig(HEAD_VERTICES, MALE_RIG_HASH).build();
    let mut entries = head_entries(Some(rig));
    entries.retain(|entry| {
        !Path::new(entry.path())
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mtl"))
    });
    let material = TestMtlMaterial::new(FALLBACK_NAME)
        .diffuse([0.1, 0.2, 0.3])
        .texture("TexSlot1", "Textures/Test/fallback_diff.dds");
    entries.push(TestEntry::zstd(DEFAULT, TestMtl::single(&material).cryxml()).encrypted());
    entries.extend(
        SplitDds::new(TestTextureFormat::LegacyBgra8, 4, 4)
            .entries("Data/Textures/Test/fallback_diff.dds"),
    );
    entries
}

#[test]
fn physical_fallback_binds_each_material_and_image_to_the_encrypted_entry() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = archive(directory.path(), entries())?;
    let character = chf(directory.path(), "character.chf", Some((MALE_RIG_HASH, 2)))?;
    let out = directory.path().join("fallback.glb");
    let key = test_key().to_hex();
    let run = glb_with(
        &archive,
        &character,
        &out,
        true,
        &["--key", &key, "--materials", "textures"],
    )?;
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    let report: Value = serde_json::from_str(&run.stdout)?;
    assert_eq!(
        report["default_material"]["state"], "resolved",
        "verified physical fallback"
    );
    assert_eq!(
        report["default_material"]["source"]["entry_path"], DEFAULT,
        "original Entry"
    );
    assert_eq!(
        report["default_material"]["source"]["contains_encrypted_source"], true,
        "encrypted fallback"
    );
    assert!(
        report["issues"]
            .as_array()
            .ok_or("authored issues")?
            .iter()
            .any(|issue| issue["kind"] == "material_library"),
        "the authored material failure stays visible"
    );
    assert_eq!(
        report
            .pointer("/fieldOrigins/~1materials/containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "the report material count is sourced from actual assignments"
    );
    let fields = &report["fieldOrigins"];
    for (address, encrypted) in [
        ("/default_material/source/entry_path", false),
        ("/default_material/decoded_sha256", true),
    ] {
        assert_eq!(
            fields[address]["containsEncryptedSource"], encrypted,
            "{address} uses its original byte domain",
        );
    }
    check_fields(&out, &report)
}

fn check_fields(out: &Path, report: &Value) -> TestResult {
    let bytes = fs::read(out)?;
    let glb = check_glb(&bytes)?;
    let material = glb
        .document
        .materials()
        .find(|material| material.name() == Some(FALLBACK_NAME))
        .ok_or("physical GLB material")?;
    let image = material
        .pbr_metallic_roughness()
        .base_color_texture()
        .ok_or("physical diffuse")?
        .texture()
        .source();
    let name = report["originReceipt"].as_str().ok_or("receipt path")?;
    let contents = fs::read_to_string(out.with_file_name(name))?;
    let rows: Vec<Value> = contents
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let material_index = material.index().ok_or("material index")?;
    for address in [
        format!("/materials/{material_index}"),
        format!("/images/{}", image.index()),
    ] {
        let row = rows
            .iter()
            .find(|row| {
                row.pointer("/address/path").and_then(Value::as_str) == Some(address.as_str())
            })
            .ok_or("original fallback field receipt")?;
        assert_eq!(
            row["containsEncryptedSource"], true,
            "{address} keeps the encrypted contributor"
        );
        assert_eq!(
            row["sourceSetComplete"], true,
            "{address} has the complete observed set"
        );
    }
    Ok(())
}
