// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestBank, TestEntry, TestHircObject};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::audio_origin_assertions as proof;
use crate::support::{Install, Server, TestResult};

const TOOL: &str = "list_soundbank_objects";
const ROOT: &str = "/structuredContent";
const ENCRYPTED: &str = "Data/Sounds/wwise/protected.bnk";
const PLAIN: &str = "Data/Sounds/wwise/plain.bnk";
const UNKNOWN: &str = "Data/Sounds/wwise/unknown.bnk";
const EMPTY: &str = "Data/Sounds/wwise/empty.bnk";
const RESPONSE_LIMIT: usize = 256 * 1024;

#[test]
fn object_source_proofs_distinguish_directory_and_body_encryption() -> TestResult {
    let bytes = TestBank::new(71)
        .version(154)
        .object(TestHircObject::event(37, &[21, 21]))
        .object(TestHircObject::raw(0xfe, 37, [0_u8; 4]))
        .bytes();
    let hirc = bytes
        .windows(4)
        .position(|word| word == b"HIRC")
        .ok_or("fixed fixture HIRC section")? as u64;
    let install = Install::new(vec![
        TestEntry::stored(ENCRYPTED, bytes.as_slice()).encrypted(),
        TestEntry::stored(PLAIN, bytes.as_slice()),
        TestEntry::stored(UNKNOWN, bytes)
            .encrypted()
            .with_encryption_field(7),
        TestEntry::stored(EMPTY, TestBank::new(72).bytes()).encrypted(),
    ])?;
    let mut server = serve(&install)?;
    let encrypted = server.ok(TOOL, &json!({"soundbank":ENCRYPTED,"type":"event","id":37}))?;
    assert_original_objects(&encrypted, hirc)?;
    validate_schema(&mut server, &encrypted)?;
    validate_receipt(&encrypted)?;
    for (path, marker) in [(PLAIN, Some(false)), (UNKNOWN, None)] {
        let report = server.ok(TOOL, &json!({"soundbank":path,"type":"event","id":37}))?;
        assert_eq!(
            report.get("objects"),
            encrypted.get("objects"),
            "metadata preserves the original object JSON"
        );
        assert_marker(&report, "/objects/0/id", marker, true)?;
        assert_marker(&report, "/soundbank", Some(false), true)?;
        validate_receipt(&report)?;
    }
    let raw = server.ok(TOOL, &json!({"soundbank":ENCRYPTED,"id":37}))?;
    assert_eq!(
        raw.pointer("/objects/1/type_byte"),
        Some(&json!(254)),
        "unknown raw type byte survives"
    );
    assert_marker(&raw, "/objects/1/type_byte", Some(true), true)?;
    assert_marker(&raw, "/objects/1/status", Some(true), true)?;
    let empty = server.ok(TOOL, &json!({"soundbank":EMPTY}))?;
    assert_eq!(
        empty.get("total"),
        Some(&json!(0)),
        "no HIRC gives zero original objects"
    );
    assert_marker(&empty, "/total", Some(true), true)?;
    assert_eq!(
        proof::uses(&empty, "/structuredContent/total")?.len(),
        1,
        "only the physical section-directory scan proves HIRC absence"
    );
    validate_receipt(&empty)
}

fn assert_original_objects(report: &Value, hirc: u64) -> TestResult {
    assert_eq!(
        report.get("version"),
        Some(&json!(154)),
        "original BKHD version"
    );
    assert_eq!(report.get("bank_id"), Some(&json!(71)), "original BKHD ID");
    assert_eq!(
        report.get("total"),
        Some(&json!(2)),
        "both physical objects counted independently of display"
    );
    assert_eq!(
        report.get("matched"),
        Some(&json!(1)),
        "same ID with an unmatched type remains excluded"
    );
    assert_eq!(
        report.get("objects"),
        Some(&json!([{
            "index":0,"id":37,"type":"event","status":"decoded","error":null,"parent":null,
            "children":[21,21],"detail":null
        }])),
        "the existing object values and repeated Event actions are unchanged"
    );
    assert_original_ranges(report, hirc)?;
    assert_marker(report, "/objects/0/children/1", Some(true), true)?;
    assert_marker(report, "/layouts/decoded", Some(true), false)?;
    assert_marker(report, "/objects/0/status", Some(true), false)?;
    let selection = proof::uses(report, "/structuredContent/matched")?;
    assert!(
        selection.iter().any(|site| {
            site.get("role").and_then(Value::as_str) == Some("selector")
                && site.pointer("/location/field").and_then(Value::as_str)
                    == Some("objectIds/event")
        }),
        "the combined filter consulted ID words only on matching Event rows"
    );
    Ok(())
}

fn assert_original_ranges(report: &Value, hirc: u64) -> TestResult {
    let first_object = hirc + 8 + 4;
    assert_range(report, "/version", "bnkBkhd", "version", 8..12)?;
    assert_range(report, "/bank_id", "bnkBkhd", "bankId", 12..16)?;
    assert_range(
        report,
        "/total",
        "bnkHirc",
        "objectCount",
        hirc + 8..hirc + 12,
    )?;
    assert_range(
        report,
        "/objects/0/id",
        "bnkHircObject",
        "id",
        first_object + 5..first_object + 9,
    )?;
    assert_range(
        report,
        "/objects/0/children/0",
        "bnkHircEvent",
        "actionId",
        first_object + 10..first_object + 14,
    )?;
    assert_range(
        report,
        "/objects/0/children/1",
        "bnkHircEvent",
        "actionId",
        first_object + 14..first_object + 18,
    )
}

#[test]
fn object_paging_accounts_for_metadata_and_keeps_global_counts() -> TestResult {
    const TOTAL: usize = 64;
    const ACTIONS: usize = 32;
    let mut bank = TestBank::new(73).version(154);
    for id in 0..u32::try_from(TOTAL)? {
        bank = bank.object(TestHircObject::event(id + 1, &[77; ACTIONS]));
    }
    let install = Install::new(vec![TestEntry::stored(ENCRYPTED, bank.bytes()).encrypted()])?;
    let mut server = serve(&install)?;
    let mut report = server.ok(TOOL, &json!({"soundbank":ENCRYPTED,"limit":TOTAL}))?;
    let mut seen = 0;
    loop {
        seen += verify_object_page(&report, seen, TOTAL, ACTIONS)?;
        let Some(cursor) = report.get("next_cursor").and_then(Value::as_str) else {
            break;
        };
        assert!(seen < TOTAL, "a next cursor means unseen objects remain");
        assert_marker(&report, "/next_cursor", Some(true), false)?;
        report = server.ok(
            TOOL,
            &json!({"soundbank":ENCRYPTED,"limit":TOTAL,"cursor":cursor}),
        )?;
    }
    assert_eq!(
        seen, TOTAL,
        "all objects remain reachable across exact byte-bounded pages"
    );
    Ok(())
}

fn verify_object_page(
    report: &Value,
    start: usize,
    total: usize,
    actions: usize,
) -> TestResult<usize> {
    let rows = report
        .get("objects")
        .and_then(Value::as_array)
        .ok_or("object page")?;
    assert!(!rows.is_empty(), "each page advances the cursor");
    assert!(
        rows.len() < total,
        "proof forces paging of otherwise small rows"
    );
    assert_eq!(
        report.get("total"),
        Some(&json!(total)),
        "global object count is stable"
    );
    assert_eq!(
        report.get("matched"),
        Some(&json!(total)),
        "global match count is stable"
    );
    assert!(
        serde_json::to_vec(report)?.len() <= RESPONSE_LIMIT,
        "structured rows and metadata fit together"
    );
    let first = rows.first().ok_or("first row")?;
    let last = rows.last().ok_or("last row")?;
    assert_eq!(
        first.get("index"),
        Some(&json!(start)),
        "paging resumes at the next original object"
    );
    assert_eq!(
        last.get("id"),
        Some(&json!(start + rows.len())),
        "the page retains original physical order"
    );
    assert_marker(
        report,
        &format!("/objects/{}/children/{}", rows.len() - 1, actions - 1),
        Some(true),
        true,
    )?;
    Ok(rows.len())
}

fn assert_marker(report: &Value, path: &str, marker: Option<bool>, complete: bool) -> TestResult {
    let origin = proof::origin(report, &format!("{ROOT}{path}"))?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(marker)),
        "{path} has its own encryption state"
    );
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(complete)),
        "{path} has its own completeness state"
    );
    Ok(())
}

fn assert_range(
    report: &Value,
    path: &str,
    namespace: &str,
    label: &str,
    bytes: std::ops::Range<u64>,
) -> TestResult {
    let expected = json!([bytes.start.to_string(), bytes.end.to_string()]);
    let uses = proof::uses(report, &format!("{ROOT}{path}"))?;
    assert!(
        uses.iter().any(|site| {
            site.get("role").and_then(Value::as_str) == Some("payload")
                && site.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && site.pointer("/location/field").and_then(Value::as_str) == Some(label)
                && site.pointer("/location/indices") == Some(&expected)
        }),
        "{path} uses the exact half-open decoded range {expected}"
    );
    Ok(())
}

fn validate_schema(server: &mut Server, report: &Value) -> TestResult {
    let tools = server.tools()?;
    let tool = tools
        .iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some(TOOL))
        .ok_or("soundbank objects tool")?;
    jsonschema::validator_for(tool.get("outputSchema").ok_or("objects output schema")?)?
        .validate(report)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn validate_receipt(report: &Value) -> TestResult {
    let fields = report
        .get("fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("field address table")?;
    for address in fields.keys() {
        let path = address.strip_prefix(ROOT).ok_or("structured result path")?;
        assert!(
            report.pointer(path).is_some(),
            "every origin points to an emitted datum: {path}"
        );
        for used in proof::uses(report, address)? {
            let source = proof::source_path(report, used)?;
            assert!(
                std::path::Path::new(source)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("bnk")),
                "every reference resolves to an actual bank source"
            );
        }
    }
    Ok(())
}
