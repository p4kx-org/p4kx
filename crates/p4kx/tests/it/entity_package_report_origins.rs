// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_testkit::{assembly_entries, test_key};
use serde_json::{Value, json};

use super::{TestResult, package, write_archive};

#[test]
fn json_reports_the_original_entity_but_keeps_package_statistics_partial() -> TestResult {
    for encrypted in [false, true] {
        let dir = tempfile::tempdir()?;
        let entries = assembly_entries()
            .into_iter()
            .map(|entry| {
                if encrypted && entry.path() == "Data\\Game2.dcb" {
                    entry.encrypted()
                } else {
                    entry
                }
            })
            .collect();
        let archive = write_archive(dir.path(), entries)?;
        let out = dir.path().join("json");
        let text_out = dir.path().join("text");
        let key = test_key().to_hex();
        let args = ["--key", key.as_str()];
        let run = package(&archive, "TEST_Ship", &out, &args, true)?;
        assert_eq!(run.code, Some(0), "{}", run.stderr);
        let report: Value = serde_json::from_str(&run.stdout)?;
        assert_eq!(
            report.get("entity").and_then(Value::as_str),
            Some("TEST_Ship"),
            "the selected Entity"
        );
        verify_report(&report, encrypted)?;

        let text = package(&archive, "TEST_Ship", &text_out, &args, false)?;
        assert_eq!(text.code, Some(0), "{}", text.stderr);
        for path in ["scene.json", "meshes/m0001.glb"] {
            assert_eq!(
                fs::read(out.join(path))?,
                fs::read(text_out.join(path))?,
                "JSON report mode preserves {path}"
            );
        }
    }
    Ok(())
}

fn verify_report(report: &Value, encrypted: bool) -> TestResult {
    let fields = report.get("fieldOrigins").ok_or("field origins")?;
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    for (path, field, role) in [
        ("/entity", "recordName", "conversion"),
        ("/guid", "guid", "payload"),
    ] {
        let origin = fields.get(path).ok_or("selected Entity field origin")?;
        assert_eq!(
            origin
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(encrypted),
            "{path}"
        );
        assert_eq!(
            origin.get("sourceSetComplete").and_then(Value::as_bool),
            Some(true),
            "{path}"
        );
        let refs = origin
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("field uses")?;
        assert!(
            uses.iter().any(|usage| {
                usage.get("id").is_some_and(|id| refs.contains(id))
                    && usage.get("role").and_then(Value::as_str) == Some(role)
                    && usage
                        .pointer("/location/recordIndex")
                        .and_then(Value::as_u64)
                        == Some(0)
                    && usage.pointer("/location/field").and_then(Value::as_str) == Some(field)
            }),
            "{path} retains the selected original Record's {field}"
        );
    }
    assert_eq!(
        fields.get("/texture_mip").ok_or("texture mip origin")?,
        &json!({"containsEncryptedSource": false, "sourceSetComplete": true, "sourceUsesRefs": []}),
        "texture mip is a tool option"
    );
    assert_eq!(
        fields.get("/nodes").ok_or("node count origin")?,
        &json!({"containsEncryptedSource": null, "sourceSetComplete": false, "sourceUsesRefs": []}),
        "the selected Record alone does not prove the package node count"
    );
    Ok(())
}
