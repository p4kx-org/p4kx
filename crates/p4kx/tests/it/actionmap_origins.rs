// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;
use std::process::Output;

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use serde_json::{Value, json};

use super::{PROFILE, TestResult, archive, english, profile, standard};

#[path = "actionmap_origin_publication.rs"]
mod publication;

fn run(archive: &Path, arguments: &[&str]) -> Result<Output, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?
        .args([
            "--key",
            &p4kx_testkit::test_key().to_hex(),
            "actionmap",
            "export",
        ])
        .arg(archive)
        .args(arguments)
        .output()?;
    assert!(output.status.success(), "{output:?}");
    Ok(output)
}

fn rows(bytes: &[u8]) -> Result<Vec<Value>, Box<dyn Error>> {
    let lines = std::str::from_utf8(bytes)?;
    let rows: Vec<Value> = lines
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    assert_eq!(
        rows.first().ok_or("header")?["format"],
        "p4kx-source-origin",
        "fixed format identifier"
    );
    assert_eq!(
        rows.last().ok_or("end")?["type"],
        "end",
        "mandatory terminal record"
    );
    for (row_type, count_field) in [
        ("source", "sources"),
        ("sourceUse", "sourceUses"),
        ("field", "fields"),
    ] {
        let count = rows.iter().filter(|row| row["type"] == row_type).count();
        assert_eq!(
            rows.last().ok_or("end")?[count_field],
            count.to_string(),
            "{row_type} count"
        );
    }
    let mut uses = std::collections::BTreeSet::new();
    for row in &rows {
        if row["type"] == "sourceUse" {
            let id = row
                .pointer("/sourceUse/id")
                .and_then(Value::as_str)
                .ok_or("use ID")?;
            assert!(uses.insert(id), "duplicate source-use ID: {id}");
        }
    }
    for row in &rows {
        if row["type"] == "field" {
            let references = row["sourceUsesRefs"].as_array().ok_or("field refs")?;
            assert!(
                references
                    .iter()
                    .all(|reference| reference.as_str().is_some_and(|id| uses.contains(id))),
                "each field use resolves within the same receipt"
            );
        }
    }
    Ok(rows)
}

fn check_binding(rows: &[Value], artifact: &[u8]) -> TestResult {
    let mut measured = OriginDigestWriter::new(Vec::new());
    std::io::Write::write_all(&mut measured, artifact)?;
    let expected = serde_json::to_value(measured.finish()?)?;
    let binding = &rows.first().ok_or("header")?["binding"];
    assert_eq!(
        binding.get("sha256"),
        expected.get("sha256"),
        "bound digest"
    );
    assert_eq!(
        binding.get("byteLength"),
        expected.get("byteLength"),
        "bound byte count"
    );
    Ok(())
}

fn field<'rows>(rows: &'rows [Value], path: &str) -> Result<&'rows Value, Box<dyn Error>> {
    let field = rows.iter().find(|row| {
        row["type"] == "field"
            && ["/address/path", "/address/field"]
                .into_iter()
                .any(|address| row.pointer(address).and_then(Value::as_str) == Some(path))
    });
    Ok(field.ok_or_else(|| format!("missing field {path}"))?)
}

fn uses<'rows>(rows: &'rows [Value], field: &Value) -> Result<Vec<&'rows Value>, Box<dyn Error>> {
    field["sourceUsesRefs"]
        .as_array()
        .ok_or("field refs")?
        .iter()
        .map(|id| {
            rows.iter()
                .find_map(|row| {
                    let source_use = row.get("sourceUse")?;
                    (source_use["id"] == *id).then_some(source_use)
                })
                .ok_or_else(|| "unresolved use".into())
        })
        .collect()
}

fn assert_json_bytes(stdout: &[u8]) {
    let expected = concat!(
        "{\"language\":\"en\",\"action_maps\":[{\"name\":\"seat_general\",",
        "\"label_key\":\"@ui_CGSeatGeneral\",\"label\":\"Map\",\"actions\":[",
        "{\"name\":\"v_eject\",\"label_key\":\"@ui_CIEject\",\"label\":\"Final\\nLabel\",",
        "\"activation_mode\":\"press\",\"bindings\":[",
        "{\"device\":\"keyboard\",\"input\":\"ralt+y\",\"activation_mode\":\"press\"},",
        "{\"device\":\"joystick\",\"input\":\"button4\",\"activation_mode\":\"press\"}]},",
        "{\"name\":\"prone\",\"label_key\":\"@ui_CIProne\",\"label\":null,",
        "\"activation_mode\":\"press\",\"bindings\":[",
        "{\"device\":\"keyboard\",\"input\":\"lctrl\",\"activation_mode\":\"press\"},",
        "{\"device\":\"gamepad\",\"input\":\"b\",\"activation_mode\":\"delayed_press\"}]},",
        "{\"name\":\"v_nolabel\",\"label_key\":\"@ui_Missing\",\"label\":null,",
        "\"activation_mode\":\"tap\",\"bindings\":[",
        "{\"device\":\"keyboard\",\"input\":\"=cmd\",\"activation_mode\":\"tap\"}]}]}]}\n",
    );
    assert_eq!(stdout, expected.as_bytes(), "legacy JSON bytes");
}

#[test]
fn json_proves_mixed_action_profile_and_language_sources() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path =
        archive(
            dir.path(),
            vec![profile(), TestEntry::stored(
        "Data/Localization/english/global.ini",
        b"ui_CGSeatGeneral=Map\nui_CIEject=First\nui_CIEject=Final\\nLabel\nui_CIProne=\n".to_vec(),
    ).encrypted()],
        )?;
    let plain = run(&path, &[])?;
    let receipt = dir.path().join("origin.jsonl");
    let actual = run(
        &path,
        &[
            "--origin-stderr",
            "--origin-out",
            receipt.to_str().ok_or("receipt path")?,
        ],
    )?;
    assert_eq!(plain.stdout, actual.stdout);
    assert_json_bytes(&actual.stdout);
    assert_eq!(actual.stderr, std::fs::read(receipt)?);
    let rows = rows(&actual.stderr)?;
    check_binding(&rows, &actual.stdout)?;
    let data: Value = serde_json::from_slice(&actual.stdout)?;
    let name = field(&rows, "/action_maps/0/actions/0/name")?;
    let label = field(&rows, "/action_maps/0/actions/0/label")?;
    assert_eq!(name["containsEncryptedSource"], false);
    assert_eq!(label["containsEncryptedSource"], true);
    assert_eq!(label["sourceSetComplete"], true);
    assert_eq!(
        data["action_maps"][0]["actions"][0]["label"],
        "Final\nLabel"
    );
    let label_uses = uses(&rows, label)?;
    assert!(label_uses.iter().any(|source| source["role"] == "selector"
        && source["location"]["namespace"] == "textXmlAttribute"));
    let payload = label_uses
        .iter()
        .find(|source| source["role"] == "payload")
        .ok_or("INI payload")?;
    let span = payload["location"]["indices"]
        .as_array()
        .ok_or("payload range")?;
    assert_eq!(span, &vec![json!("49"), json!("61")]);
    assert!(
        label_uses
            .iter()
            .any(|source| source["role"] == "conversion")
    );
    check_unresolved_labels(&rows)
}

fn check_unresolved_labels(rows: &[Value]) -> TestResult {
    let missing = field(rows, "/action_maps/0/actions/2/label")?;
    assert!(
        uses(rows, missing)?.iter().any(|source| source
            .pointer("/location/type")
            .and_then(Value::as_str)
            == Some("localizationLookup")),
        "a missing key identifies the exact original selector"
    );
    let empty = field(rows, "/action_maps/0/actions/1/label")?;
    assert!(
        uses(rows, empty)?
            .iter()
            .all(|source| source.get("role").and_then(Value::as_str) != Some("payload")),
        "an empty value is consulted but supplies no label bytes"
    );
    Ok(())
}

#[test]
fn csv_proves_written_offsets_and_preserves_legacy_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let path = standard(dir.path())?;
    for (extra, expected) in [
        (vec!["--format", "csv"], "'=cmd"),
        (vec!["--format", "csv", "--csv-raw"], "=cmd"),
    ] {
        let legacy = run(&path, &extra)?;
        let actual = run(&path, &[extra, vec!["--origin-stderr"]].concat())?;
        assert_eq!(
            legacy.stdout, actual.stdout,
            "CSV bytes with origin enabled"
        );
        let rows = rows(&actual.stderr)?;
        check_binding(&rows, &actual.stdout)?;
        for (path, expected) in [
            ("/rows/1/label", "\"Prone, \"\"flat\"\"\""),
            ("/rows/1/gamepad", "b (delayed_press)"),
            ("/rows/2/keyboard", expected),
            ("/rows/2/mouse", ""),
        ] {
            let address = &field(&rows, path)?["address"];
            let start: usize = address["start"].as_str().ok_or("start")?.parse()?;
            let end: usize = address["endExclusive"].as_str().ok_or("end")?.parse()?;
            assert_eq!(
                actual.stdout.get(start..end).ok_or("CSV address")?,
                expected.as_bytes(),
                "{path}"
            );
        }
    }
    Ok(())
}

#[test]
fn cryxml_keeps_physical_attributes_for_inherited_and_overridden_modes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let profile = TestElement::new("profile").child(
        TestElement::new("actionmap")
            .attribute("name", "map")
            .child(
                TestElement::new("action")
                    .attribute("name", "action")
                    .attribute("keyboard", "f")
                    .attribute("activationMode", "tap")
                    .attribute("UILabel", "@LOC_EMPTY")
                    .child(TestElement::new("keyboard").attribute("input", "f"))
                    .child(
                        TestElement::new("gamepad")
                            .attribute("activationMode", "hold")
                            .child(TestElement::new("inputdata").attribute("input", "button")),
                    ),
            ),
    );
    let bytes = profile.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let path = archive(
        dir.path(),
        vec![
            TestEntry::stored("Data/Libs/Config/defaultProfile.xml", bytes),
            english(),
        ],
    )?;
    let actual = run(&path, &["--origin-stderr"])?;
    let rows = rows(&actual.stderr)?;
    let parent = "/action_maps/0/actions/0";
    let inherited = field(&rows, &format!("{parent}/bindings/0/activation_mode"))?;
    let overridden = field(&rows, &format!("{parent}/bindings/1/activation_mode"))?;
    let direct = field(&rows, &format!("{parent}/activation_mode"))?;
    assert_eq!(direct["sourceUsesRefs"], inherited["sourceUsesRefs"]);
    assert_ne!(direct["sourceUsesRefs"], overridden["sourceUsesRefs"]);
    let data: Value = serde_json::from_slice(&actual.stdout)?;
    assert_eq!(
        data["action_maps"][0]["actions"][0]["bindings"]
            .as_array()
            .ok_or("bindings")?
            .len(),
        2
    );
    assert!(
        uses(&rows, overridden)?
            .iter()
            .all(|source| source["location"]["namespace"] == "cryxmlAttribute")
    );
    let placeholder = field(&rows, &format!("{parent}/label"))?;
    assert!(
        uses(&rows, placeholder)?
            .iter()
            .all(|source| source["location"]["namespace"] == "cryxmlAttribute")
    );
    Ok(())
}

#[test]
fn equal_artifact_bytes_replace_the_prior_file_origin() -> TestResult {
    let dir = tempfile::tempdir()?;
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::create_dir(&first)?;
    std::fs::create_dir(&second)?;
    let first = standard(&first)?;
    let second = archive(
        &second,
        vec![
            TestEntry::stored(
                "Data/Libs/Config/defaultProfile.xml",
                PROFILE.as_bytes().to_vec(),
            )
            .encrypted(),
            english(),
        ],
    )?;
    let output = dir.path().join("map.json");
    let receipt = dir.path().join("map.json.p4kx-origin.jsonl");
    let arguments = ["--out", output.to_str().ok_or("output")?];
    run(&first, &arguments)?;
    let old_artifact = std::fs::read(&output)?;
    let old = rows(&std::fs::read(&receipt)?)?;
    let report = run(
        &second,
        &["--json", "--out", output.to_str().ok_or("output")?],
    )?;
    let report: Value = serde_json::from_slice(&report.stdout)?;
    assert_eq!(report["origin_receipt"], "map.json.p4kx-origin.jsonl");
    let new = rows(&std::fs::read(&receipt)?)?;
    let name = "/action_maps/0/actions/0/name";
    assert_eq!(std::fs::read(output)?, old_artifact);
    assert_eq!(
        old.first().ok_or("old header")?["binding"],
        new.first().ok_or("new header")?["binding"],
        "equal bytes give equal bindings"
    );
    assert_eq!(field(&old, name)?["containsEncryptedSource"], false);
    assert_eq!(field(&new, name)?["containsEncryptedSource"], true);
    Ok(())
}
