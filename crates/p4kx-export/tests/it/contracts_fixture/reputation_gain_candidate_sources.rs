// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataForgeError, Value};
use p4kx_testkit::{TestRecord, TestValue};
use serde_json::{Value as Json, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

use super::super::{RANK_TWO_HUNDRED, lookups};
use super::{CONTRACT_A, assert_site, assert_use, database, source};

const WIDE: &str = "9007199254740993";
const WIDE_TYPE: &str = "ReputationRewardWide";
const LATER_AMOUNT: i64 = 1_007;
const LATER_NAME: &str = "ReputationRewardWide.later_probe";
const LATER_FILE: &str =
    "libs/foundry/records/reputation/rewards/missionrewards_reputation/later_probe.xml";

#[test]
fn reached_nominal_reward_candidates_keep_their_physical_header_selectors() -> TestResult {
    let bytes = source::with_duplicate_reward(database()).build();
    let fixture = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    let forge = fixture.entry.open()?;
    let original = source::read(&forge)?;
    let wide = forge
        .records()
        .find(|record| record.type_name().is_ok_and(|name| name == WIDE_TYPE))
        .ok_or("actual selected wide Reward Record")?;
    assert_eq!(
        wide.guid().to_string(),
        health_origin_fixture::guid(RANK_TWO_HUNDRED),
        "the selected physical Reward shares the authored nominal GUID"
    );
    let (_, catalog) = fixture.diagnostic()?;
    let amount = amount_cell(&catalog)?;
    assert_eq!(
        amount.get("sourceSetComplete"),
        Some(&json!(true)),
        "both reached original candidates are readable"
    );
    let uses = origin::uses(&catalog, amount)?;
    assert_site(&uses, original.gains[0].reward.caller, "selector");
    assert_site(&uses, original.gains[0].amount, "payload");
    for index in [wide.index(), original.later_empty] {
        for field in ["guid", "recordIndex", "fileName"] {
            assert_use(&uses, "dataforgeRecordHeader", index, field, "selector");
        }
    }
    assert!(
        uses.iter()
            .all(|used| !at_header(used, original.gains[0].reward.target)),
        "the lower decoder-resolved duplicate is outside the reached nominal selection"
    );
    origin::closure(&catalog)
}

#[test]
fn unreadable_later_reward_file_name_preserves_the_amount_without_certifying_coverage() -> TestResult
{
    let mut bytes = source::with_duplicate_reward(database())
        .record(later_candidate())
        .build();
    let index = check_readable_candidate(&bytes)?;
    corrupt_token(&mut bytes, LATER_FILE)?;
    let fixture = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    let forge = fixture.entry.open()?;
    let failed = forge
        .record(index)
        .ok_or("actual unreadable Reward candidate")?;
    assert!(
        matches!(
            failed.record_path(),
            Err(DataForgeError::InvalidString { .. })
        ),
        "the physical candidate fails only its authored fileName"
    );
    let original = source::read(&forge)?;
    assert!(
        original.gains[0].reward.target < index && original.later_empty < index,
        "the caller still resolves to the lower physical target and the uncertainty is reached first"
    );
    let (_, catalog) = fixture.diagnostic()?;
    check_incomplete_amount(&catalog, &original, index)?;
    origin::closure(&catalog)
}

#[test]
fn unreadable_selected_reward_name_keeps_whole_record_attempt_only() -> TestResult {
    let mut bytes = source::with_duplicate_reward(database())
        .record(later_candidate())
        .build();
    let index = check_readable_candidate(&bytes)?;
    corrupt_token(&mut bytes, LATER_NAME)?;
    let fixture = Fixture::open(&bytes, Input::Plain, Vec::new())?;
    let forge = fixture.entry.open()?;
    let failed = forge
        .record(index)
        .ok_or("actual selected physical Reward candidate")?;
    assert_eq!(
        failed.stored_file_name()?,
        LATER_FILE,
        "the damaged candidate still qualifies as a Reward by its actual filename"
    );
    let Err(DataForgeError::InvalidString { table, offset }) = failed.stored_name() else {
        return Err("the actual RecordName must fail in the original table".into());
    };
    assert!(
        matches!(
            failed.tree(),
            Err(DataForgeError::InvalidString {
                table: read_table,
                offset: read_offset,
            }) if read_table == table && read_offset == offset
        ),
        "the whole Record read reports the same metadata failure"
    );

    let (_, catalog) = fixture.diagnostic()?;
    let amount = incomplete_amount(&catalog)?;
    let original = source::read(&forge)?;
    let uses = origin::uses(&catalog, amount)?;
    assert_site(&uses, original.gains[0].reward.caller, "selector");
    assert_site(&uses, original.gains[0].amount, "payload");
    assert_whole_record_attempt(&catalog, index)?;
    origin::closure(&catalog)
}

fn assert_whole_record_attempt(catalog: &Json, index: u32) -> TestResult {
    let uses = origin::rows(catalog, "/provenance/uses")?;
    let whole_record = json!({"type": "dataForge", "recordIndex": index, "field": ""});
    assert!(
        uses.iter().any(|used| {
            used.get("role").and_then(Json::as_str) == Some("attempted")
                && used.get("location") == Some(&whole_record)
        }),
        "the failed candidate contributes the original whole-Record read attempt"
    );
    assert!(
        uses.iter().all(|used| {
            used.pointer("/location/namespace").and_then(Json::as_str)
                != Some("dataforgeRecordBody")
                || used.pointer("/location/indices") != Some(&json!([index.to_string()]))
        }),
        "the RecordName failure precedes any body read of this physical Reward"
    );
    Ok(())
}

fn later_candidate() -> TestRecord {
    lookups::plain_record(
        RANK_TWO_HUNDRED,
        WIDE_TYPE,
        "reputation/rewards/missionrewards_reputation/later_probe",
    )
    .value("reputationAmount", TestValue::Int64(LATER_AMOUNT))
}

fn check_readable_candidate(bytes: &[u8]) -> TestResult<u32> {
    let forge = DataForge::open(bytes)?;
    let record = forge
        .records()
        .last()
        .ok_or("actual latest physical Reward")?;
    assert_eq!(
        record.stored_file_name()?,
        LATER_FILE,
        "the appended candidate has the actual Reward selector prefix"
    );
    assert_eq!(
        record.guid().to_string(),
        health_origin_fixture::guid(RANK_TWO_HUNDRED),
        "the appended candidate belongs to the authored nominal GUID"
    );
    assert_eq!(
        record.tree()?.field("reputationAmount"),
        Some(&Value::Int64(LATER_AMOUNT)),
        "the unmodified later candidate has its own different usable value"
    );
    Ok(record.index())
}

fn corrupt_token(bytes: &mut [u8], token: &str) -> TestResult {
    let mut matches = bytes
        .windows(token.len())
        .enumerate()
        .filter_map(|(index, stored)| (stored == token.as_bytes()).then_some(index));
    let position = matches.next().ok_or("actual candidate header token")?;
    assert!(
        matches.next().is_none(),
        "only one actual header token matches {token}"
    );
    *bytes.get_mut(position).ok_or("actual header token byte")? = 0xff;
    Ok(())
}

fn incomplete_amount(catalog: &Json) -> TestResult<&Json> {
    let amount = amount_cell(catalog)?;
    assert_eq!(
        amount.get("sourceSetComplete"),
        Some(&json!(false)),
        "the unknown higher candidate prevents a complete decision"
    );
    assert_eq!(
        amount.get("containsEncryptedSource"),
        Some(&Json::Null),
        "readable plain contributors alone do not certify a partial result as plain"
    );
    Ok(amount)
}

fn check_incomplete_amount(catalog: &Json, original: &source::Original, index: u32) -> TestResult {
    let amount = incomplete_amount(catalog)?;
    let uses = origin::uses(catalog, amount)?;
    assert_site(&uses, original.gains[0].reward.caller, "selector");
    assert_site(&uses, original.gains[0].amount, "payload");
    for field in ["guid", "recordIndex"] {
        assert_use(&uses, "dataforgeRecordHeader", index, field, "selector");
    }
    assert!(
        uses.iter().all(|used| {
            !at_header(used, index) || used.pointer("/location/field") != Some(&json!("fileName"))
        }),
        "a failed path is not an immediate contributor"
    );
    let inventory = origin::rows(catalog, "/provenance/uses")?;
    let refs: Vec<_> = inventory.iter().collect();
    assert_use(
        &refs,
        "dataforgeRecordHeader",
        index,
        "fileName",
        "attempted",
    );
    Ok(())
}

fn amount_cell(catalog: &Json) -> TestResult<&Json> {
    let row = catalog_contracts::entry(catalog, &health_origin_fixture::guid(CONTRACT_A), None)?;
    let list = origin::cell(row, "reputationGains", "present")?;
    let [first, _] = origin::rows(list, "/value")? else {
        return Err("original successful gain order".into());
    };
    let amount = origin::cell(first, "amount", "present")?;
    assert_eq!(
        amount.get("value"),
        Some(&json!(WIDE)),
        "the legacy selected amount is unchanged"
    );
    Ok(amount)
}

fn at_header(used: &Json, index: u32) -> bool {
    used.pointer("/location/namespace").and_then(Json::as_str) == Some("dataforgeRecordHeader")
        && used.pointer("/location/indices") == Some(&json!([index.to_string()]))
}
