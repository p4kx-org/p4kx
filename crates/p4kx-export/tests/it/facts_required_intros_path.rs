// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForgeError;
use p4kx_testkit::{TestInstance, TestRecord, TestValue};
use serde_json::{Value, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

use super::super::{FAIL_TAG, GATED, database};
use super::{intro, requirements};

const PATH: &str = "libs/foundry/records/contracts/contractgenerator/hidden_facts.xml";
const CANDIDATE: u128 = 0x2500_0000_0000_4000_8000_0000_0010;
const GRANTER: u128 = 0x2100_0000_0000_4000_8000_0000_0012;
const GENERATOR: &str = "ContractGenerator";
const OTHER_TYPE: &str = "ContractTemplate";
const INPUT: Input = Input::Encrypted;

#[test]
fn an_unreadable_generator_path_cannot_prove_the_intro_grant_set_is_complete() -> TestResult {
    let clean = database().record(generator()).build();
    let readable = Fixture::open(&clean, INPUT, Vec::new())?;
    check_original(&readable, GENERATOR, Some(PATH))?;
    let (_, catalog) = readable.diagnostic()?;
    let known = origin::rows(grants(&catalog)?, "/value")?;
    let [grant] = known else {
        return Err("the readable physical Generator supplies exactly one matching intro".into());
    };
    assert_eq!(
        grant.get("value"),
        Some(&json!(health_origin_fixture::guid(GRANTER))),
        "the unchanged body grants the actual required tag when its path is readable"
    );

    let fixture = Fixture::open(&damage_path(clean)?, INPUT, Vec::new())?;
    let index = check_original(&fixture, GENERATOR, None)?;
    let (_, catalog) = fixture.diagnostic()?;
    let missing = grants(&catalog)?;
    assert!(
        origin::rows(missing, "/value")?.is_empty(),
        "selection cannot publish the unadmitted intro ID"
    );
    assert_eq!(
        missing.get("sourceSetComplete"),
        Some(&json!(false)),
        "the unreadable path leaves this original Generator's grants unresolved"
    );
    check_coverage(&catalog, missing, index)?;
    origin::closure(&catalog)
}

#[test]
fn an_unreadable_unrelated_path_does_not_make_intro_grants_incomplete() -> TestResult {
    let record = TestRecord::new(CANDIDATE, OTHER_TYPE, "ContractTemplate.Unrelated", PATH);
    let bytes = damage_path(database().record(record).build())?;
    let fixture = Fixture::open(&bytes, INPUT, Vec::new())?;
    let index = check_original(&fixture, OTHER_TYPE, None)?;
    let (_, catalog) = fixture.diagnostic()?;
    let cell = grants(&catalog)?;
    assert!(
        origin::rows(cell, "/value")?.is_empty(),
        "the original unrelated type supplies no intro IDs"
    );
    assert_eq!(
        cell.get("sourceSetComplete"),
        Some(&json!(true)),
        "the readable non-Generator type rules out grant membership"
    );
    assert!(
        origin::uses(&catalog, cell)?.iter().all(
            |used| !at_header(used, index, "sourceType") && !at_header(used, index, "fileName")
        ),
        "the unrelated Record supplies no selected grant input"
    );
    origin::closure(&catalog)
}

fn generator() -> TestRecord {
    let handler = TestInstance::new("ContractHandler_Career").value(
        "introContracts",
        TestValue::Array(vec![intro(GRANTER, "HiddenIntro", &[true], FAIL_TAG)]),
    );
    TestRecord::new(CANDIDATE, GENERATOR, "ContractGenerator.Hidden", PATH).value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}

fn damage_path(mut bytes: Vec<u8>) -> TestResult<Vec<u8>> {
    let mut matching = bytes
        .windows(PATH.len())
        .enumerate()
        .filter_map(|(index, token)| (token == PATH.as_bytes()).then_some(index));
    let position = matching.next().ok_or("original physical fileName token")?;
    assert!(
        matching.next().is_none(),
        "the test mutates only the unique Record fileName"
    );
    *bytes
        .get_mut(position)
        .ok_or("original physical fileName byte")? = 0xff;
    Ok(bytes)
}

fn check_original(
    fixture: &Fixture,
    source_type: &str,
    file_name: Option<&str>,
) -> TestResult<u32> {
    let forge = fixture.entry.open()?;
    let record = forge.record(4).ok_or("actual appended original Record")?;
    assert_eq!(
        record.guid().to_string(),
        health_origin_fixture::guid(CANDIDATE),
        "the candidate is the expected physical Record"
    );
    assert_eq!(
        record.type_name()?,
        source_type,
        "the independent original type remains readable"
    );
    if let Some(file_name) = file_name {
        assert_eq!(
            record.stored_file_name()?,
            file_name,
            "the clean candidate is at the authored Generator path"
        );
    } else {
        assert!(
            matches!(
                record.stored_file_name(),
                Err(DataForgeError::InvalidString { .. })
            ),
            "the original fileName getter fails on the actual damaged token"
        );
    }
    Ok(record.index())
}

fn grants(catalog: &Value) -> TestResult<&Value> {
    let contract = catalog_contracts::entry(catalog, &health_origin_fixture::guid(GATED), None)?;
    let required = json!(health_origin_fixture::guid(FAIL_TAG));
    let requirement = requirements(contract)?
        .iter()
        .find(|row| row.pointer("/tagGuid/value") == Some(&required))
        .ok_or("original selected prerequisite tag")?;
    origin::cell(requirement, "introContracts", "present")
}

fn check_coverage(catalog: &Value, cell: &Value, index: u32) -> TestResult {
    let retained = origin::uses(catalog, cell)?;
    let selected_type = retained
        .iter()
        .find(|used| {
            at_header(used, index, "sourceType") && used.get("role") == Some(&json!("selector"))
        })
        .ok_or("the actual Generator type affects incomplete membership")?;
    assert!(
        retained
            .iter()
            .all(|used| !at_header(used, index, "fileName")),
        "the unreadable path attempt is excluded from contributing references"
    );
    let attempt = origin::rows(catalog, "/provenance/uses")?
        .iter()
        .find(|used| {
            at_header(used, index, "fileName") && used.get("role") == Some(&json!("attempted"))
        })
        .ok_or("the actual Generator fileName failure is retained")?;
    assert_eq!(
        origin::source(catalog, selected_type)?,
        origin::source(catalog, attempt)?,
        "the observed type and the failed path belong to the same original DCB"
    );
    Ok(())
}

fn at_header(used: &Value, index: u32, field: &str) -> bool {
    used.pointer("/location/namespace").and_then(Value::as_str) == Some("dataforgeRecordHeader")
        && used.pointer("/location/indices") == Some(&json!([index.to_string()]))
        && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
}
