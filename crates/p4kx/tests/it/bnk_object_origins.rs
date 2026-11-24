// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ops::Range;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, TestHircObject};
use serde_json::{Value, json};

use super::{TestResult, assert_field, assert_reference_closure, report_uses};

const ENTRY: &str = "Data/Sounds/selected.bnk";
const CLEAR_ENTRY: &str = "Data/Sounds/clear.bnk";
const OBJECT_ID: u32 = 0x0102_0304;
const UNKNOWN_TYPE: u8 = 0xFE;
const UNKNOWN_TYPE_RANGE: Range<usize> = 78..79;
const UNKNOWN_ID_RANGE: Range<usize> = 83..87;
const SOUND_TYPE_RANGE: Range<usize> = 91..92;
const SOUND_ID_RANGE: Range<usize> = 96..100;

#[test]
fn the_selected_physical_bank_and_paged_objects_keep_original_words() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("fixture.p4k");
    let file = directory.path().join("standalone.bnk");
    let bytes = bank_fixture();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(CLEAR_ENTRY, bytes.clone()))
        .entry(TestEntry::stored(ENTRY, bytes.clone()).encrypted())
        .entry(TestEntry::stored(ENTRY, bytes.clone()))
        .write_to(&archive)?;
    std::fs::write(&file, &bytes)?;
    let selectors = ["--id", "0x01020304", "--offset", "1", "--limit", "1"];

    let encrypted = objects(&archive, Some(ENTRY), &selectors)?;
    assert_encrypted_page(&encrypted)?;

    let plain = objects(&archive, Some(CLEAR_ENTRY), &selectors)?;
    assert_original_words(
        &plain,
        (UNKNOWN_TYPE_RANGE, UNKNOWN_ID_RANGE),
        Some(false),
        "archiveEntry",
    )?;

    let standalone = objects(&file, None, &selectors)?;
    assert_eq!(
        standalone.get("objects"),
        encrypted.get("objects"),
        "identical bank bytes retain the same object page"
    );
    assert_field(&standalone, "/soundbank", None, true)?;
    assert_field(&standalone, "/objects/0/type_byte", None, true)?;
    assert_original_words(
        &standalone,
        (UNKNOWN_TYPE_RANGE, UNKNOWN_ID_RANGE),
        None,
        "file",
    )?;

    let sounds = objects(
        &archive,
        Some(ENTRY),
        &["--type", "sound", "--id", "0x01020304", "--limit", "1"],
    )?;
    assert_json_cells(
        &sounds,
        &[
            ("/matched", json!(1)),
            ("/objects/0/index", json!(2)),
            ("/objects/0/type", json!("sound")),
        ],
    );
    assert!(
        sounds.pointer("/objects/0/type_byte").is_none()
            && sounds
                .pointer("/fieldOrigins/~1objects~10~1type_byte")
                .is_none(),
        "a recognized type emits neither an extra raw byte nor its carrier"
    );
    assert_original_words(
        &sounds,
        (SOUND_TYPE_RANGE, SOUND_ID_RANGE),
        Some(true),
        "archiveEntry",
    )
}

fn bank_fixture() -> Vec<u8> {
    let bytes = TestBank::new(57)
        .version(154)
        .object(TestHircObject::event(OBJECT_ID, &[42]))
        .object(TestHircObject::raw(UNKNOWN_TYPE, OBJECT_ID, [0; 4]))
        .object(TestHircObject::sound(OBJECT_ID, 81, 2, 4096))
        .bytes();
    let identity = OBJECT_ID.to_le_bytes();
    for (range, word) in [
        (UNKNOWN_TYPE_RANGE, [UNKNOWN_TYPE].as_slice()),
        (UNKNOWN_ID_RANGE, identity.as_slice()),
        (SOUND_TYPE_RANGE, [2].as_slice()),
        (SOUND_ID_RANGE, identity.as_slice()),
    ] {
        assert_eq!(
            bytes.get(range.clone()),
            Some(word),
            "authored fixture word at {range:?}"
        );
    }
    bytes
}

fn assert_encrypted_page(encrypted: &Value) -> TestResult {
    assert_json_cells(
        encrypted,
        &[
            ("/soundbank", json!(ENTRY)),
            ("/version", json!(154)),
            ("/bank_id", json!(57)),
            ("/matched", json!(3)),
            ("/next_offset", json!(2)),
            ("/objects/0/index", json!(1)),
            ("/objects/0/id", json!(OBJECT_ID)),
            ("/objects/0/type", json!("other")),
            ("/objects/0/type_byte", json!(UNKNOWN_TYPE)),
            ("/provenance/sources/0/entryIndex", json!(1)),
            ("/provenance/sources/0/path/value", json!(ENTRY)),
        ],
    );
    assert_field(encrypted, "/soundbank", Some(false), true)?;
    let path = report_uses(encrypted, "/soundbank")?;
    let [path] = path.as_slice() else {
        return Err("one bank path contributor".into());
    };
    assert_json_cells(
        path,
        &[
            ("/scope", json!("archiveDirectoryMetadata")),
            ("/location", json!({"type":"metadata","field":"path"})),
        ],
    );
    assert_original_words(
        encrypted,
        (UNKNOWN_TYPE_RANGE, UNKNOWN_ID_RANGE),
        Some(true),
        "archiveEntry",
    )?;
    assert_field(encrypted, "/objects/0/type_byte", Some(true), true)?;
    assert_range(
        encrypted,
        "/objects/0/type_byte",
        "payload",
        "bnkHircObject",
        "typeCode",
        UNKNOWN_TYPE_RANGE,
    )?;

    Ok(())
}

fn assert_json_cells(report: &Value, expected: &[(&str, Value)]) {
    for (pointer, value) in expected {
        assert_eq!(
            report.pointer(pointer),
            Some(value),
            "the expected field at {pointer}"
        );
    }
}

fn objects(path: &Path, entry: Option<&str>, selectors: &[&str]) -> Result<Value, Box<dyn Error>> {
    let mut command = p4kx_testkit::p4kx()?;
    command.args(["--json", "bnk", "objects"]).arg(path);
    if let Some(entry) = entry {
        command
            .arg(entry)
            .env("P4KX_ARCHIVE_KEY", p4kx_testkit::test_key().to_hex());
    }
    let output = command.args(selectors).output()?;
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let report = serde_json::from_slice(&output.stdout)?;
    assert_reference_closure(&report)?;
    Ok(report)
}

fn assert_original_words(
    report: &Value,
    words: (Range<usize>, Range<usize>),
    encrypted: Option<bool>,
    source_kind: &str,
) -> TestResult {
    for path in ["/version", "/bank_id", "/objects/0/id", "/objects/0/type"] {
        assert_field(report, path, encrypted, true)?;
    }
    let (kind, id) = words;
    for (path, role, namespace, field, range) in [
        ("/version", "payload", "bnkBkhd", "version", 8..12),
        ("/bank_id", "payload", "bnkBkhd", "bankId", 12..16),
        ("/objects/0/id", "payload", "bnkHircObject", "objectId", id),
        (
            "/objects/0/type",
            "conversion",
            "bnkHircObject",
            "typeCode",
            kind,
        ),
        ("/objects/0/type", "selector", "bnkBkhd", "version", 8..12),
    ] {
        assert_range(report, path, role, namespace, field, range)?;
    }
    let sources = report
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    let [source] = sources.as_slice() else {
        return Err("one physical bank observation".into());
    };
    assert_eq!(
        source.get("kind").and_then(Value::as_str),
        Some(source_kind),
        "the selected bank has the expected physical source kind"
    );
    let source_id = source.get("id").ok_or("physical bank observation ID")?;
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("uses")?;
    for contributor in uses {
        assert_eq!(
            contributor.get("source"),
            Some(source_id),
            "the selected bank"
        );
    }
    Ok(())
}

fn assert_range(
    report: &Value,
    path: &str,
    role: &str,
    namespace: &str,
    field: &str,
    range: Range<usize>,
) -> TestResult {
    let expected = json!([range.start.to_string(), range.end.to_string()]);
    assert!(
        report_uses(report, path)?.iter().any(|site| {
            site["role"] == role
                && site["scope"] == "payload"
                && site.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && site.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && site.pointer("/location/indices") == Some(&expected)
        }),
        "{path} binds original {role} {field} at {expected}"
    );
    Ok(())
}
