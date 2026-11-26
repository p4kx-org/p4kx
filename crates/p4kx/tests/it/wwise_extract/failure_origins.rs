// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAtl, TestBank, TestEntity, TestEntry, TestHircObject,
    audio_schema, entity_schema, wwise_short_id,
};
use serde_json::{Value, json};

use super::{EMBEDDED, Fixture, PLAY, STREAMED, TestResult, extract, pcm};

const TRIGGER: &str = "Play_FailureOrigins";
const ENTITY: &str = "TEST_FailureOrigins";
const FILELESS_ID: u32 = 770;
const MISSING_WEM: u32 = 701;
const BLOCKED_WEM: u32 = 702;

fn authored_fixture(events: &[&str]) -> Result<Fixture, Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let archive = folder.path().join("Data.p4k");
    let out = folder.path().join("out");
    let bank = TestBank::new(wwise_short_id("FailureOrigins"))
        .wem(BLOCKED_WEM, pcm())
        .object(TestHircObject::event(wwise_short_id(TRIGGER), &[1, 2, 3]))
        .object(TestHircObject::action(1, PLAY, 11))
        .object(TestHircObject::action(2, PLAY, 12))
        .object(TestHircObject::action(3, PLAY, 13))
        .object(TestHircObject::plugin_sound(11, FILELESS_ID))
        .object(TestHircObject::sound(12, MISSING_WEM, STREAMED, 0))
        .object(TestHircObject::sound(13, BLOCKED_WEM, EMBEDDED, 0));
    let entity = TestEntity::new(
        0x6840_b276_310f_4f11_90af_953b_bfc4_b823,
        ENTITY,
        "libs/foundry/records/entities/spaceships/failure_origins.xml",
    )
    .audio_trigger("A_Unmatched")
    .audio_trigger(TRIGGER);
    let database = audio_schema(entity_schema(DataForgeBuilder::new())).record(entity.record());
    ArchiveBuilder::v2()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()).encrypted())
        .entry(TestEntry::zstd(
            "Data\\Libs\\GameAudio\\FailureOrigins.xml",
            TestAtl::new("FailureOrigins")
                .trigger(TRIGGER, events)
                .to_cryxml(),
        ))
        .entry(TestEntry::zstd("Data\\Sounds\\wwise\\FailureOrigins.bnk", bank.bytes()).encrypted())
        .write_to(&archive)?;
    fs::create_dir_all(out.join(format!("{TRIGGER}_{BLOCKED_WEM}.wem")))?;
    Ok(Fixture {
        _dir: folder,
        archive,
        out,
    })
}

fn fixture() -> Result<Fixture, Box<dyn Error>> {
    authored_fixture(&[TRIGGER])
}

#[test]
fn wwise_failures_keep_original_variants_and_separate_operational_errors() -> TestResult {
    let fixture = fixture()?;
    let report = extract(
        &fixture,
        TRIGGER,
        &["--format", "raw", "--conflict", "overwrite"],
        1,
    )?;
    check_failures(&report, "")
}

#[test]
fn entity_failures_use_the_same_origins_after_an_unmatched_trigger() -> TestResult {
    let fixture = fixture()?;
    let output = p4kx_testkit::p4kx()?
        .arg("--json")
        .args(["entity", "audio"])
        .arg(&fixture.archive)
        .arg(ENTITY)
        .arg("--extract")
        .arg(&fixture.out)
        .args(["--format", "raw", "--conflict", "overwrite"])
        .output()?;
    assert_eq!(output.status.code(), Some(1), "exit code: {output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        report.pointer("/triggers/0/name").and_then(Value::as_str),
        Some("A_Unmatched"),
        "the unmatched Entity trigger did not take an export position"
    );
    let selector = origin(&report, "/extraction/failures/0/trigger")?;
    assert_eq!(
        selector
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "the compacted failure position retains its original encrypted Entity selection"
    );
    check_failures(&report, "/extraction")
}

#[test]
fn txtp_skip_reason_cites_the_first_failed_leaf() -> TestResult {
    let fixture = fixture()?;
    let report = extract(
        &fixture,
        TRIGGER,
        &["--format", "raw", "--conflict", "overwrite", "--txtp"],
        1,
    )?;
    let skip = &report["txtp"]["skips"][0];
    assert_eq!(skip["event"], TRIGGER);
    let expected = format!("plug-in source {FILELESS_ID} has no file");
    assert_eq!(skip["reason"].as_str(), Some(expected.as_str()));
    let event = origin(&report, "/txtp/skips/0/event")?;
    assert_eq!(event["containsEncryptedSource"], false);
    let reason = origin(&report, "/txtp/skips/0/reason")?;
    assert_eq!(reason["containsEncryptedSource"], true);
    assert_eq!(reason["sourceSetComplete"], true);
    assert_eq!(reason, origin(&report, "/failures/0/reason")?);
    Ok(())
}

#[test]
fn equal_event_names_keep_distinct_authored_skip_positions() -> TestResult {
    let fixture = authored_fixture(&[TRIGGER, TRIGGER])?;
    let report = extract(
        &fixture,
        TRIGGER,
        &["--format", "raw", "--conflict", "overwrite", "--txtp"],
        1,
    )?;
    assert_eq!(report["txtp"]["skipped"], 2);
    let first = origin(&report, "/txtp/skips/0/event")?;
    let second = origin(&report, "/txtp/skips/1/event")?;
    assert_eq!(first, origin(&report, "/trigger/events/0/name")?);
    assert_eq!(second, origin(&report, "/trigger/events/1/name")?);
    assert_ne!(first["sourceUsesRefs"], second["sourceUsesRefs"]);
    Ok(())
}

#[test]
fn txtp_publication_skip_keeps_event_but_has_unknown_operational_reason() -> TestResult {
    let fixture = super::fixture()?;
    fs::create_dir_all(fixture.out.join("Play_Single.txtp"))?;
    let report = extract(
        &fixture,
        "Play_Single",
        &["--format", "wav", "--conflict", "overwrite", "--txtp"],
        0,
    )?;
    assert_eq!(report["txtp"]["skipped"], 1);
    assert_eq!(report["txtp"]["skips"][0]["event"], "Play_Single");
    assert_eq!(report["written"], 1);
    let reason = origin(&report, "/txtp/skips/0/reason")?;
    assert_eq!(reason["containsEncryptedSource"], Value::Null);
    assert_eq!(reason["sourceSetComplete"], false);
    assert_eq!(reason["sourceUsesRefs"], json!([]));
    Ok(())
}

fn check_failures(report: &Value, root: &str) -> TestResult {
    let expected_ids = [Value::Null, json!(MISSING_WEM), json!(BLOCKED_WEM)];
    let failures = report
        .pointer(&format!("{root}/failures"))
        .and_then(Value::as_array)
        .ok_or("failure rows")?;
    assert_eq!(
        failures.len(),
        expected_ids.len(),
        "one row per actual failure"
    );
    for (ordinal, (row, id)) in failures.iter().zip(&expected_ids).enumerate() {
        assert_eq!(
            row.get("wem_id"),
            Some(id),
            "original ordering and nullable WEM ID"
        );
        let path = format!("{root}/failures/{ordinal}/wem_id");
        let evidence = origin(report, &path)?;
        assert_eq!(
            evidence
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true),
            "the reported WEM field keeps the original terminal selection"
        );
    }
    check_failure_reason_origins(report, root)
}

fn check_failure_reason_origins(report: &Value, root: &str) -> TestResult {
    let declared_reason = origin(report, &format!("{root}/failures/0/reason"))?;
    let absent_id = origin(report, &format!("{root}/failures/0/wem_id"))?;
    assert_eq!(
        declared_reason
            .get("sourceSetComplete")
            .and_then(Value::as_bool),
        Some(true),
        "fileless reason is sourced by its authored plug-in and source ID"
    );
    assert!(
        has_declared_id(report, declared_reason)?,
        "the reason uses the actual declared source ID"
    );
    assert!(
        !has_declared_id(report, absent_id)?,
        "the nullable WEM ID does not claim the unused declaration"
    );
    let unresolved = origin(report, &format!("{root}/failures/1/reason"))?;
    assert_eq!(
        unresolved.get("sourceSetComplete").and_then(Value::as_bool),
        Some(false),
        "a missing search trace leaves incomplete evidence"
    );
    let job_reason = origin(report, &format!("{root}/failures/2/reason"))?;
    assert_eq!(
        job_reason.get("containsEncryptedSource"),
        Some(&Value::Null),
        "filesystem reason is unknown"
    );
    assert_eq!(
        job_reason.get("sourceUsesRefs"),
        Some(&json!([])),
        "the reason does not cite attempted media"
    );
    Ok(())
}

fn has_declared_id(report: &Value, evidence: &Value) -> Result<bool, Box<dyn Error>> {
    let references = evidence
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("origin refs")?;
    let inventory = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("use inventory")?;
    Ok(inventory.iter().any(|site| {
        site.get("id").is_some_and(|id| references.contains(id))
            && site.pointer("/location/field").and_then(Value::as_str) == Some("sourceId")
    }))
}

fn origin<'a>(report: &'a Value, path: &str) -> Result<&'a Value, Box<dyn Error>> {
    report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .ok_or_else(|| format!("missing origin for {path}").into())
}
