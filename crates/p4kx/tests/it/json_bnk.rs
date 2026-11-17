// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The `--json` output of `p4kx bnk`, as snapshots.

use std::collections::BTreeSet;
use std::error::Error;
use std::path::Path;

use p4kx_testkit::{TestBank, TestCodec, TestWem, test_signal};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn write_bank(dir: &Path) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let file = dir.join("SSAM_Test.bnk");
    let bank = TestBank::new(0xD155_6D49)
        .wem(
            100,
            TestWem::pcm(TestCodec::WwisePcm, 2, 48_000, &test_signal(1_200, 2)).bytes(),
        )
        .wem(
            300,
            TestWem::encoded(TestCodec::Vorbis, 1, 44_100, 88_200).bytes(),
        )
        .wem(400, b"RIFF\x04\0\0\0WAVE".to_vec())
        .wem(
            500,
            TestWem::encoded(TestCodec::Vorbis, 1, 44_100, 88_200)
                .bytes()
                .get(..120)
                .unwrap_or_default()
                .to_vec(),
        )
        .bytes();
    std::fs::write(&file, bank)?;
    Ok(file)
}

fn json(dir: &Path, args: &[&std::ffi::OsStr]) -> Result<String, Box<dyn Error>> {
    let output = p4kx_testkit::p4kx()?.arg("--json").args(args).output()?;
    let mut value: Value = serde_json::from_slice(&output.stdout)?;
    let prefix = dir.display().to_string();
    redact(&mut value, &prefix);
    Ok(serde_json::to_string_pretty(&value)?)
}

fn redact(value: &mut Value, prefix: &str) {
    match value {
        Value::String(text) => {
            if let Some(rest) = text.strip_prefix(prefix) {
                *text = format!("[DIR]{}", rest.replace('\\', "/"));
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| redact(item, prefix)),
        Value::Object(fields) => fields.values_mut().for_each(|item| redact(item, prefix)),
        _ => {}
    }
}

#[test]
fn bnk_list() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_bank(dir.path())?;

    let json = json(
        dir.path(),
        &["bnk".as_ref(), "list".as_ref(), file.as_os_str()],
    )?;

    let mut value: Value = serde_json::from_str(&json)?;
    let report = value.as_object_mut().ok_or("bank list response object")?;
    report.remove("fieldOrigins");
    report.remove("provenance");
    let json = serde_json::to_string_pretty(&value)?;
    insta::assert_snapshot!(json);
    Ok(())
}

#[test]
fn bnk_extract() -> TestResult {
    let dir = tempfile::tempdir()?;
    let file = write_bank(dir.path())?;
    let out = dir.path().join("out");

    let json = json(
        dir.path(),
        &[
            "bnk".as_ref(),
            "extract".as_ref(),
            file.as_os_str(),
            "--out".as_ref(),
            out.as_os_str(),
        ],
    )?;

    let mut value: Value = serde_json::from_str(&json)?;
    let extracted = value["extracted"]
        .as_array_mut()
        .ok_or("extracted raw media")?;
    for row in extracted {
        let row = row.as_object_mut().ok_or("raw media row")?;
        row.remove("originReceipt");
        row.remove("originReceiptDigest");
    }
    insta::assert_snapshot!(serde_json::to_string_pretty(&value)?);
    Ok(())
}

#[test]
fn bnk_list_origin_distinguishes_physical_archive_and_standalone_fields() -> TestResult {
    use p4kx_testkit::{ArchiveBuilder, TestEntry};

    let temp = tempfile::tempdir()?;
    let bank = temp.path().join("standalone.bnk");
    let archive = temp.path().join("fixture.p4k");
    let bytes = origin_bank_bytes()?;
    std::fs::write(&bank, &bytes)?;
    let encrypted_path = "Data/Sounds/protected.bnk";
    let plain_path = "Data/Sounds/plain.bnk";
    ArchiveBuilder::new()
        .entry(TestEntry::stored(encrypted_path, bytes.clone()).encrypted())
        .entry(TestEntry::stored(plain_path, bytes))
        .write_to(&archive)?;

    let encrypted = list_origin_fixture(&archive, Some(encrypted_path))?;
    assert_encrypted_header(&encrypted)?;
    assert_encrypted_wems(&encrypted)?;

    let plain = list_origin_fixture(&archive, Some(plain_path))?;
    let standalone = list_origin_fixture(&bank, None)?;
    let expected = at(&encrypted, "/wems")?;
    assert_eq!(at(&plain, "/wems")?, expected, "clear decoded WEM values");
    assert_eq!(
        at(&standalone, "/wems")?,
        expected,
        "standalone decoded WEM values"
    );
    assert_field(&plain, "/wems/0/codec", Some(false), true)?;
    assert_standalone_source(&standalone)?;

    for report in [&encrypted, &plain, &standalone] {
        assert_reference_closure(report)?;
    }
    Ok(())
}

fn origin_bank_bytes() -> Result<Vec<u8>, Box<dyn Error>> {
    let sound = TestWem::pcm(TestCodec::Pcm, 1, 8000, &[0, 1, 2, 3]).bytes();
    let prefetch = sound.get(..12).ok_or("RIFF prefix")?.to_vec();
    Ok(TestBank::new(57)
        .wem(9, b"PLUG".to_vec())
        .wem(1, sound)
        .wem(8, prefetch)
        .wem(4, b"RIFF\x04\x00\x00\x00WAVE".to_vec())
        .wem(1, b"PLUG".to_vec())
        .bytes())
}

fn list_origin_fixture(file: &Path, entry: Option<&str>) -> Result<Value, Box<dyn Error>> {
    let mut cmd = p4kx_testkit::p4kx()?;
    cmd.args(["--json", "bnk", "list"]).arg(file);
    if let Some(entry) = entry {
        cmd.arg(entry)
            .env("P4KX_ARCHIVE_KEY", p4kx_testkit::test_key().to_hex());
    }
    let output = cmd.output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn assert_encrypted_header(report: &Value) -> TestResult {
    assert_field(report, "/soundbank", Some(false), true)?;
    for path in ["/version", "/bank_id", "/wems"] {
        assert_field(report, path, Some(true), true)?;
    }
    for (path, namespace, field, indices) in [
        ("/version", "bnkBkhd", "version", ["8", "12"]),
        ("/bank_id", "bnkBkhd", "bankId", ["12", "16"]),
        ("/wems/0/id", "bnkDidx", "mediaId", ["72", "76"]),
        ("/wems/0/size", "bnkDidx", "dataLength", ["80", "84"]),
        ("/wems/0/class", "bnkWemRiffHeader", "form", ["152", "156"]),
    ] {
        assert_site(report, path, namespace, field, &json!(indices))?;
    }
    Ok(())
}

fn assert_encrypted_wems(report: &Value) -> TestResult {
    let rows = at(report, "/wems")?.as_array().ok_or("WEM rows")?;
    let [pcm, broken, prefix, plugin] = rows.as_slice() else {
        return Err("one physical WEM row per selected ID".into());
    };
    assert_eq!(at(pcm, "/codec")?, "pcm", "the first physical ID wins");
    for field in [
        "class",
        "codec",
        "codec_name",
        "sample_count",
        "duration_seconds",
        "prefetch",
    ] {
        assert_field(report, &format!("/wems/0/{field}"), Some(true), true)?;
    }
    assert!(at(broken, "/error")?.is_string(), "malformed WEM diagnosed");
    assert_eq!(at(broken, "/codec")?, &Value::Null, "no parsed codec");
    assert_field(report, "/wems/1/error", Some(true), false)?;
    assert_field(report, "/wems/1/codec", Some(true), false)?;
    assert_eq!(at(prefix, "/prefetch")?, true, "retained RIFF prefix");
    assert_eq!(at(prefix, "/codec")?, &Value::Null, "no parsed codec");
    assert_field(report, "/wems/2/codec", Some(true), true)?;
    assert!(
        report_uses(report, "/wems/2/prefetch")?
            .iter()
            .any(
                |used| used.pointer("/location/field").and_then(Value::as_str)
                    == Some("declaredSize")
            ),
        "prefetch retains its original declared length selector"
    );
    assert_eq!(at(plugin, "/class")?, "impulse", "stored plug-in signature");
    assert_eq!(
        at(plugin, "/error")?,
        &Value::Null,
        "a skipped class is not a decode failure"
    );
    assert_field(report, "/wems/3/codec", Some(true), true)?;
    Ok(())
}

fn assert_standalone_source(report: &Value) -> TestResult {
    assert_field(report, "/version", None, true)?;
    let contributions = report_uses(report, "/version")?;
    let [header] = contributions.as_slice() else {
        return Err("one BKHD version contributor".into());
    };
    let observed_file = at(header, "/source")?;
    let sources = at(report, "/provenance/sources")?
        .as_array()
        .ok_or("sources")?;
    assert!(
        sources.iter().any(|source| {
            source.get("id") == Some(observed_file)
                && source.get("kind").and_then(Value::as_str) == Some("file")
        }),
        "standalone references identify their observed file"
    );
    Ok(())
}

fn assert_reference_closure(report: &Value) -> TestResult {
    let origins = at(report, "/fieldOrigins")?
        .as_object()
        .ok_or("field origins")?;
    let mut referenced = BTreeSet::new();
    for path in origins.keys() {
        assert!(report.pointer(path).is_some(), "unbound origin {path}");
    }
    collect_inventory_references(at(report, "/fieldOrigins")?, &mut referenced)?;
    collect_inventory_references(at(report, "/provenance/sources")?, &mut referenced)?;
    let inventory = at(report, "/provenance/uses")?
        .as_array()
        .ok_or("inventory uses")?;
    let published = inventory
        .iter()
        .map(|used| {
            at(used, "/id")?
                .as_str()
                .ok_or_else(|| "published use ID".into())
        })
        .collect::<Result<BTreeSet<_>, Box<dyn Error>>>()?;
    assert_eq!(published.len(), inventory.len(), "unique use rows");
    assert_eq!(
        referenced, published,
        "field and physical-source metadata closure"
    );
    Ok(())
}

fn collect_inventory_references<'a>(value: &'a Value, refs: &mut BTreeSet<&'a str>) -> TestResult {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_inventory_references(item, refs)?;
            }
        }
        Value::Object(fields) => {
            for (name, value) in fields {
                if name == "sourceUsesRefs" {
                    for id in value.as_array().ok_or("source metadata references")? {
                        refs.insert(id.as_str().ok_or("source metadata use ID")?);
                    }
                } else {
                    collect_inventory_references(value, refs)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn at<'a>(value: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing {path}").into())
}

fn assert_field(report: &Value, path: &str, encrypted: Option<bool>, complete: bool) -> TestResult {
    let origin = at(report, "/fieldOrigins")?
        .get(path)
        .ok_or_else(|| format!("missing origin for {path}"))?;
    assert_eq!(
        at(origin, "/containsEncryptedSource")?,
        &json!(encrypted),
        "{path}"
    );
    assert_eq!(at(origin, "/sourceSetComplete")?, complete, "{path}");
    Ok(())
}

fn report_uses<'a>(report: &'a Value, path: &str) -> Result<Vec<&'a Value>, Box<dyn Error>> {
    let origin = at(report, "/fieldOrigins")?
        .get(path)
        .ok_or_else(|| format!("missing origin for {path}"))?;
    let refs = at(origin, "/sourceUsesRefs")?
        .as_array()
        .ok_or("source use references")?;
    let inventory = at(report, "/provenance/uses")?
        .as_array()
        .ok_or("source use inventory")?;
    refs.iter()
        .map(|id| {
            inventory
                .iter()
                .find(|used| used.get("id") == Some(id))
                .ok_or_else(|| format!("unresolved source use {id}").into())
        })
        .collect()
}

fn assert_site(
    report: &Value,
    path: &str,
    namespace: &str,
    field: &str,
    indices: &Value,
) -> TestResult {
    assert!(
        report_uses(report, path)?.iter().any(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && used.pointer("/location/indices") == Some(indices)
        }),
        "{path} has original {field} at {indices}"
    );
    Ok(())
}
