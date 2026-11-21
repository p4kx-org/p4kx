// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_testkit::{TestBank, TestCodec, TestEntry, TestWem};
use serde_json::{Value, json};

use crate::audio::serve;
use crate::support::{Install, Server, TestResult};

const TOOL: &str = "list_soundbank_wems";
const ENCRYPTED: &str = "Data/Sounds/wwise/protected.bnk";
const PLAIN: &str = "Data/Sounds/wwise/plain.bnk";
const EMPTY: &str = "Data/Sounds/wwise/no-media.bnk";
const RESPONSE_LIMIT: usize = 256 * 1024;

fn pcm() -> Vec<u8> {
    TestWem::pcm(TestCodec::Pcm, 1, 8000, &[0, 1, 2, 3]).bytes()
}

#[test]
fn listed_wem_fields_keep_their_physical_words_and_failure_evidence() -> TestResult {
    let bank = bank_with_mixed_media()?;
    let install = Install::new(vec![
        TestEntry::deflated(ENCRYPTED, bank).encrypted(),
        TestEntry::stored(PLAIN, TestBank::new(58).wem(1, pcm()).bytes()),
        TestEntry::deflated(EMPTY, TestBank::new(59).bytes()).encrypted(),
    ])?;
    let mut server = serve(&install)?;
    let report = server.ok(TOOL, &json!({"soundbank": ENCRYPTED}))?;
    validate_receipt(&report)?;
    validate_schema(&mut server, &report)?;
    assert_bank_fields(&report)?;
    assert_parsed_sound(&report)?;
    assert_unparsed_media(&report)?;

    let plain = server.ok(TOOL, &json!({"soundbank": PLAIN}))?;
    assert_marker(&plain, "/wems/0/codec", Some(false), true)?;
    let empty = server.ok(TOOL, &json!({"soundbank": EMPTY}))?;
    assert_empty_bank(&empty)?;
    validate_receipt(&empty)
}

fn bank_with_mixed_media() -> TestResult<Vec<u8>> {
    let sound = pcm();
    let prefetch = sound.get(..12).ok_or("RIFF prefix")?.to_vec();
    Ok(TestBank::new(57)
        .wem(9, b"PLUG".to_vec())
        .wem(1, sound)
        .wem(8, prefetch)
        .wem(4, b"RIFF\x04\x00\x00\x00WAVE".to_vec())
        .wem(1, b"PLUG".to_vec())
        .bytes())
}

fn validate_schema(server: &mut Server, report: &Value) -> TestResult {
    let schema = server
        .tools()?
        .into_iter()
        .find(|tool| tool.get("name").and_then(Value::as_str) == Some(TOOL))
        .ok_or("WEM list tool")?;
    jsonschema::validator_for(schema.get("outputSchema").ok_or("output schema")?)?
        .validate(report)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn assert_bank_fields(report: &Value) -> TestResult {
    assert_eq!(
        report.get("total").and_then(Value::as_u64),
        Some(4),
        "the decoded list keeps the first duplicate"
    );
    for (row, id) in [1, 4, 8, 9].into_iter().enumerate() {
        assert_eq!(
            report
                .pointer(&format!("/wems/{row}/id"))
                .and_then(Value::as_u64),
            Some(id),
            "ID order"
        );
    }
    assert_marker(report, "/soundbank", Some(false), true)?;
    assert_marker(report, "/total", Some(true), true)?;
    assert_location(
        report,
        "/version",
        "bnkBkhd",
        "version",
        &json!(["8", "12"]),
    )?;
    assert_location(
        report,
        "/bank_id",
        "bnkBkhd",
        "bankId",
        &json!(["12", "16"]),
    )
}

fn assert_parsed_sound(report: &Value) -> TestResult {
    assert_location(
        report,
        "/wems/0/id",
        "bnkDidx",
        "mediaId",
        &json!(["72", "76"]),
    )?;
    assert_location(
        report,
        "/wems/0/size",
        "bnkDidx",
        "dataLength",
        &json!(["80", "84"]),
    )?;
    assert_eq!(
        report.pointer("/wems/0/codec").and_then(Value::as_str),
        Some("pcm"),
        "the first duplicate is still selected"
    );
    assert_marker(report, "/wems/0/channels", Some(true), true)?;
    let channels = locations(report, "/wems/0/channels")?;
    let format = channels
        .iter()
        .find(|site| {
            site.get("namespace").and_then(Value::as_str) == Some("bnkWem.wemChunk")
                && site.get("field").and_then(Value::as_str) == Some("channels")
        })
        .ok_or("the selected WEM's format channels")?;
    assert_eq!(
        format.pointer("/indices/0").and_then(Value::as_str),
        Some("144"),
        "DIDX extent, then offset inside WEM"
    );
    assert_eq!(
        format.pointer("/indices/2").and_then(Value::as_str),
        Some("20"),
        "format chunk body offset"
    );
    Ok(())
}

fn assert_unparsed_media(report: &Value) -> TestResult {
    assert_eq!(
        report.pointer("/wems/1/codec"),
        Some(&Value::Null),
        "failed parse has no parsed codec"
    );
    assert!(
        report
            .pointer("/wems/1/error")
            .is_some_and(Value::is_string),
        "the failed parse is reported"
    );
    assert_marker(report, "/wems/1/error", Some(true), false)?;
    assert_marker(report, "/wems/1/codec", Some(true), false)?;
    assert_eq!(
        report.pointer("/wems/2/prefetch").and_then(Value::as_bool),
        Some(true),
        "the RIFF size exceeds this stored extent"
    );
    assert_eq!(
        report.pointer("/wems/2/codec"),
        Some(&Value::Null),
        "a prefetch has no parsed codec"
    );
    assert_marker(report, "/wems/2/codec", Some(true), true)?;
    assert!(
        locations(report, "/wems/2/prefetch")?
            .iter()
            .any(|site| site.get("field").and_then(Value::as_str) == Some("declaredSize")),
        "use the declared size"
    );
    assert_eq!(
        report
            .pointer("/wems/3/plugin_media")
            .and_then(Value::as_bool),
        Some(true),
        "the signature selects plug-in media"
    );
    assert_eq!(
        report.pointer("/wems/3/error"),
        Some(&Value::Null),
        "plug-in media is not a failed WEM"
    );
    assert_marker(report, "/wems/3/plugin_media", Some(true), true)
}

fn assert_empty_bank(report: &Value) -> TestResult {
    assert_eq!(report.get("wems"), Some(&json!([])), "empty bank");
    assert_marker(report, "/total", Some(true), true)?;
    let sections = locations(report, "/total")?;
    let [directory] = sections.as_slice() else {
        return Err("one directory contributor".into());
    };
    assert_eq!(
        directory.get("namespace").and_then(Value::as_str),
        Some("bnkSectionDirectory"),
        "absence of DIDX is proved by the directory"
    );
    Ok(())
}

#[test]
fn the_cursor_continues_after_the_last_wem_whose_receipt_fits() -> TestResult {
    let count = 160;
    let sound = pcm();
    let bank = (1..=count)
        .rev()
        .fold(TestBank::new(60), |bank, id| bank.wem(id, sound.clone()))
        .bytes();
    let install = Install::new(vec![TestEntry::stored(PLAIN, bank)])?;
    let mut server = serve(&install)?;
    let mut next: Option<String> = None;
    let mut expected = 1;
    loop {
        let page = server.ok(
            TOOL,
            &json!({"soundbank": PLAIN, "limit": count, "cursor": next}),
        )?;
        assert!(
            serde_json::to_vec(&page)?.len() <= RESPONSE_LIMIT,
            "the entire response fits"
        );
        validate_receipt(&page)?;
        let rows = page
            .get("wems")
            .and_then(Value::as_array)
            .ok_or("page rows")?;
        assert!(!rows.is_empty(), "every page made progress");
        if expected == 1 {
            assert!(
                rows.len() < count as usize,
                "evidence consumes the first page budget"
            );
        }
        for row in rows {
            assert_eq!(
                row.get("id").and_then(Value::as_u64),
                Some(u64::from(expected)),
                "continue with the first unreturned WEM"
            );
            expected += 1;
        }
        next = page
            .get("next_cursor")
            .and_then(Value::as_str)
            .map(str::to_owned);
        if next.is_none() {
            break;
        }
        assert_marker(&page, "/next_cursor", Some(false), true)?;
        assert!(expected <= count, "a cursor requires at least one more WEM");
    }
    assert_eq!(expected, count + 1, "every WEM was returned");
    Ok(())
}

fn validate_receipt(report: &Value) -> TestResult {
    for key in report.as_object().ok_or("structured reply")?.keys() {
        if !matches!(key.as_str(), "fieldOrigins" | "provenance") {
            origin(report, &format!("/{key}"))?;
        }
    }
    let rows = report
        .get("wems")
        .and_then(Value::as_array)
        .ok_or("WEM rows")?;
    for (position, row) in rows.iter().enumerate() {
        for key in row.as_object().ok_or("WEM object")?.keys() {
            origin(report, &format!("/wems/{position}/{key}"))?;
        }
    }
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    let possible = uses
        .iter()
        .filter_map(|site| site.get("id").and_then(Value::as_str))
        .collect::<HashSet<_>>();
    let fields = report
        .get("fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("field origins")?;
    for address in fields.keys() {
        assert!(report.pointer(subpath(address)?).is_some(), "{address}");
    }
    let mut referenced = HashSet::new();
    collect_source_refs(report, &mut referenced)?;
    let unused: Vec<_> = uses
        .iter()
        .filter(|used| {
            used.get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| !referenced.contains(id))
        })
        .take(8)
        .collect();
    assert!(unused.is_empty(), "unreferenced uses: {unused:?}");
    assert!(referenced.is_subset(&possible), "unresolved field use");
    Ok(())
}

fn collect_source_refs<'a>(value: &'a Value, references: &mut HashSet<&'a str>) -> TestResult {
    match value {
        Value::Object(fields) => {
            if let Some(items) = fields.get("sourceUsesRefs") {
                for item in items.as_array().ok_or("source references")? {
                    references.insert(item.as_str().ok_or("source reference identity")?);
                }
            }
            for child in fields.values() {
                collect_source_refs(child, references)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_source_refs(item, references)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn subpath(address: &str) -> TestResult<&str> {
    address
        .strip_prefix("/structuredContent")
        .ok_or_else(|| "complete result pointer expected".into())
}

fn assert_marker(
    report: &Value,
    path: &str,
    encrypted: Option<bool>,
    complete: bool,
) -> TestResult {
    let field = origin(report, path)?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&json!(encrypted)),
        "{path}"
    );
    assert_eq!(
        field.get("sourceSetComplete").and_then(Value::as_bool),
        Some(complete),
        "{path}"
    );
    Ok(())
}

fn origin<'a>(report: &'a Value, path: &str) -> TestResult<&'a Value> {
    report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(format!("/structuredContent{path}")))
        .ok_or_else(|| format!("missing origin for {path}").into())
}

fn locations<'a>(report: &'a Value, path: &str) -> TestResult<Vec<&'a Value>> {
    let refs = origin(report, path)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source references")?;
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    refs.iter()
        .map(|id| {
            uses.iter()
                .find(|site| site.get("id") == Some(id))
                .and_then(|site| site.get("location"))
                .ok_or_else(|| format!("unresolved or unlocated source use {id}").into())
        })
        .collect()
}

fn assert_location(
    report: &Value,
    path: &str,
    namespace: &str,
    field: &str,
    indices: &Value,
) -> TestResult {
    assert!(
        locations(report, path)?.iter().any(|site| {
            site.get("namespace").and_then(Value::as_str) == Some(namespace)
                && site.get("field").and_then(Value::as_str) == Some(field)
                && site.get("indices") == Some(indices)
        }),
        "{path} should identify {namespace}/{field} {indices}"
    );
    Ok(())
}
