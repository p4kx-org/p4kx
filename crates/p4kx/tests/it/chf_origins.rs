// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_assembly::character::{Guid, lowered_hash, name_hash, skin_attachment};
use p4kx_testkit::{
    ArchiveBuilder, DRESSED_HEAD_MATERIAL_FILE, DataForgeBuilder, TestChf, TestEntity, TestEntry,
    TestItemPort, TestMaterial, TestPort, TestSubMaterial, dressed_entries, entity_schema, p4kx,
    test_key,
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

const CUSTOM_SUB: &str = "SyntheticOriginMaterial";
const CUSTOM_PARAMETER: &str = "SyntheticOriginParameter";

fn receipt(bytes: &[u8]) -> Result<Vec<Value>, serde_json::Error> {
    bytes
        .split(|&byte| byte == b'\n')
        .filter(|row| !row.is_empty())
        .map(serde_json::from_slice)
        .collect()
}

fn field<'rows>(rows: &'rows [Value], path: &str) -> Result<&'rows Value, Box<dyn Error>> {
    rows.iter()
        .find(|row| row.pointer("/address/path").and_then(Value::as_str) == Some(path))
        .ok_or_else(|| format!("missing field {path}").into())
}

fn write_mixed_fixture(archive: &Path, chf: &Path, corrupt: bool) -> TestResult {
    let forge = dressed_entries(None)
        .into_iter()
        .find(|entry| entry.path() == "Data\\Game2.dcb")
        .ok_or("fixture DataForge")?;
    let material_path = format!("Data\\{}", DRESSED_HEAD_MATERIAL_FILE.replace('/', "\\"));
    let material = format!(
        r#"<Material><SubMaterials><Material Name="{CUSTOM_SUB}"><PublicParams DyeAmount="0" {CUSTOM_PARAMETER}="1"/></Material><Material Name="{CUSTOM_SUB}"><PublicParams {CUSTOM_PARAMETER}="2"/></Material></SubMaterials></Material>"#
    );
    let selected = TestEntry::stored(&material_path, material.as_bytes()).encrypted();
    let selected = if corrupt {
        selected.with_payload([0; 16])
    } else {
        selected
    };
    ArchiveBuilder::new()
        .entry(forge)
        .entry(selected)
        .entry(TestEntry::stored(
            &material_path,
            b"<Material Name='unused'/>",
        ))
        .write_to(archive)?;
    let material_guid: Guid = "00000000-0000-0000-0000-000000000061".parse()?;
    let mut sample = TestChf::v7();
    sample.materials = vec![TestMaterial {
        attachment: skin_attachment("head_itemport"),
        base_material: material_guid.stored(),
        flags: 0,
        sub_materials: vec![TestSubMaterial {
            name: lowered_hash(CUSTOM_SUB),
            textures: Vec::new(),
            floats: vec![
                (name_hash("DyeAmount"), 0),
                (name_hash(CUSTOM_PARAMETER), 0),
            ],
            colors: Vec::new(),
        }],
    }];
    fs::write(chf, sample.to_bytes())?;
    Ok(())
}

#[test]
fn chf_label_origins_use_only_the_winning_archive_names() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("fixture.p4k");
    let chf = dir.path().join("input.chf");
    let out = dir.path().join("output.json");
    write_mixed_fixture(&archive, &chf, false)?;
    let run = p4kx()?
        .args(["--json", "--key", &test_key().to_hex(), "chf", "to-json"])
        .arg(&chf)
        .arg("--archive")
        .arg(&archive)
        .arg("--out")
        .arg(&out)
        .output()?;
    assert!(run.status.success(), "{run:?}");
    let result = fs::read(out)?;
    let stdout = p4kx()?
        .args(["--key", &test_key().to_hex(), "chf", "to-json"])
        .arg(&chf)
        .arg("--archive")
        .arg(&archive)
        .output()?;
    assert!(stdout.status.success(), "{stdout:?}");
    assert_eq!(stdout.stdout, result, "companion preserves exact CHF JSON");
    let summary: Value = serde_json::from_slice(&run.stdout)?;
    assert_eq!(summary["originReceipt"], "output.json.p4kx-origin.jsonl");
    assert_eq!(
        summary["fieldOrigins"]["/named"]["containsEncryptedSource"],
        true
    );
    assert_eq!(
        summary["fieldOrigins"]["/versions/male"]["containsEncryptedSource"],
        Value::Null
    );
    let rows = receipt(&fs::read(dir.path().join("output.json.p4kx-origin.jsonl"))?)?;
    for (pointer, origin) in [
        ("/character/versions/male", Value::Null),
        ("/character/materials/0/base_material", Value::Null),
        ("/character/materials/0/sub_materials/0/name", Value::Null),
        (
            "/character/materials/0/sub_materials/0/name_label",
            json!(true),
        ),
        (
            "/character/materials/0/sub_materials/0/floats/0/name_label",
            Value::Null,
        ),
        (
            "/character/materials/0/sub_materials/0/floats/1/name_label",
            json!(true),
        ),
    ] {
        assert_eq!(
            field(&rows, pointer)?["containsEncryptedSource"],
            origin,
            "{pointer}"
        );
    }
    check_material_parameter_site(&rows)?;
    Ok(())
}

fn check_material_parameter_site(rows: &[Value]) -> TestResult {
    let selected = field(
        rows,
        "/character/materials/0/sub_materials/0/floats/1/name_label",
    )?;
    let contributors = selected["sourceUsesRefs"].as_array().ok_or("field uses")?;
    let material_sites: Vec<_> = rows
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|datum| datum.get("id").is_some_and(|id| contributors.contains(id)))
        .filter_map(|datum| {
            datum
                .pointer("/location/namespace")
                .filter(|name| name.as_str() == Some("mtlMaterial"))
                .map(|_| datum)
        })
        .collect();
    assert_eq!(
        material_sites.len(),
        1,
        "only one physical selected name occurrence"
    );
    let material = material_sites.first().ok_or("material use")?;
    assert_eq!(
        material.pointer("/location/indices"),
        Some(&json!(["0", "1"])),
        "original material and parameter ordinal"
    );
    assert_eq!(
        material.get("effectiveEncryption"),
        Some(&json!("encrypted")),
        "the winning material name comes from the encrypted physical Entry"
    );
    Ok(())
}

#[test]
fn chf_receipts_keep_the_raw_stream_and_binary_round_trip() -> TestResult {
    let dir = tempfile::tempdir()?;
    let original = TestChf::v8().to_bytes();
    let chf = dir.path().join("input.chf");
    let json = dir.path().join("character.json");
    let durable = dir.path().join("stdout-origin.jsonl");
    let back = dir.path().join("back.chf");
    fs::write(&chf, &original)?;
    let legacy = p4kx()?.args(["chf", "to-json"]).arg(&chf).output()?;
    assert!(legacy.status.success(), "{legacy:?}");
    assert!(String::from_utf8_lossy(&legacy.stderr).contains("hashes named"));
    let with_origin = p4kx()?
        .args(["chf", "to-json"])
        .arg(&chf)
        .arg("--origin-stderr")
        .arg("--origin-out")
        .arg(&durable)
        .output()?;
    assert!(with_origin.status.success(), "{with_origin:?}");
    assert_eq!(legacy.stdout, with_origin.stdout);
    assert_eq!(with_origin.stderr, fs::read(&durable)?);
    let stdout = receipt(&with_origin.stderr)?;
    assert_eq!(
        stdout.first().ok_or("header")?.pointer("/binding/type"),
        Some(&json!("stdout"))
    );
    assert_eq!(
        field(&stdout, "/character/versions/male")?["containsEncryptedSource"],
        Value::Null
    );
    fs::write(&json, with_origin.stdout)?;

    let restored = p4kx()?
        .args(["--json", "chf", "from-json"])
        .arg(&json)
        .arg("--out")
        .arg(&back)
        .output()?;
    assert!(restored.status.success(), "{restored:?}");
    assert_eq!(fs::read(back)?, original);
    let binary = receipt(&fs::read(dir.path().join("back.chf.p4kx-origin.jsonl"))?)?;
    let payload = binary
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("opaque output datum")?;
    assert_eq!(payload["containsEncryptedSource"], Value::Null);
    assert_eq!(binary.last().ok_or("footer")?["fields"], "1");
    Ok(())
}

#[test]
fn selected_port_labels_keep_the_original_dataforge_record_and_array_slots() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("fixture.p4k");
    let input = dir.path().join("port.chf");
    let output = dir.path().join("port.json");
    let name = "SyntheticOriginPort";
    let owner = TestEntity::new(1, "SyntheticOwner", "entities/origin/owner.xml")
        .port(TestPort::new("head_itemport"))
        .port(TestPort::new(name));
    let forge = entity_schema(DataForgeBuilder::new())
        .record(owner.record())
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", forge).encrypted())
        .write_to(&archive)?;
    let owner: Guid = "00000000-0000-0000-0000-000000000001".parse()?;
    let mut chf = TestChf::v7();
    chf.ports = Some(TestItemPort {
        port: lowered_hash(name),
        item: owner.stored(),
        tail_count: 0,
        children: Vec::new(),
    });
    fs::write(&input, chf.to_bytes())?;
    let run = p4kx()?
        .args(["chf", "to-json"])
        .arg(&input)
        .arg("--archive")
        .arg(&archive)
        .arg("--out")
        .arg(&output)
        .output()?;
    assert!(run.status.success(), "{run:?}");
    let rows = receipt(&fs::read(dir.path().join("port.json.p4kx-origin.jsonl"))?)?;
    let label = field(&rows, "/character/item_ports/port_label")?;
    assert_eq!(label["containsEncryptedSource"], true);
    assert_eq!(
        field(&rows, "/character/item_ports/port")?["containsEncryptedSource"],
        Value::Null
    );
    let references = label["sourceUsesRefs"]
        .as_array()
        .ok_or("port label references")?;
    let selected = rows
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|datum| datum.get("id").is_some_and(|id| references.contains(id)))
        .find(|datum| datum.pointer("/location/type").and_then(Value::as_str) == Some("dataForge"))
        .ok_or("selected DataForge field")?;
    assert_eq!(selected["location"]["recordIndex"], 0);
    assert_eq!(selected["location"]["field"], "Components[0].Ports[1].Name");
    Ok(())
}

#[test]
fn an_unread_encrypted_name_is_attempted_without_becoming_a_field_contributor() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("fixture.p4k");
    let input = dir.path().join("input.chf");
    let output = dir.path().join("output.json");
    write_mixed_fixture(&archive, &input, true)?;
    let run = p4kx()?
        .args(["chf", "to-json"])
        .arg(&input)
        .arg("--archive")
        .arg(&archive)
        .arg("--out")
        .arg(&output)
        .output()?;
    assert!(run.status.success(), "{run:?}");
    let rows = receipt(&fs::read(dir.path().join("output.json.p4kx-origin.jsonl"))?)?;
    let attempted = rows
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .find(|datum| datum.get("role").and_then(Value::as_str) == Some("attempted"))
        .ok_or("missing attempted material observation")?;
    assert_eq!(attempted["effectiveEncryption"], "encrypted");
    let id = attempted.get("id").ok_or("attempt reference")?;
    for row in rows
        .iter()
        .filter(|row| row.get("type").and_then(Value::as_str) == Some("field"))
    {
        assert_ne!(
            row["containsEncryptedSource"], true,
            "no field uses the failed source"
        );
        assert!(
            !row["sourceUsesRefs"]
                .as_array()
                .ok_or("field references")?
                .contains(id),
            "attempts remain separate from contributions"
        );
    }
    let payload: Value = serde_json::from_slice(&fs::read(output)?)?;
    assert_eq!(
        payload.pointer("/character/materials/0/sub_materials/0/name_label"),
        None
    );
    Ok(())
}
