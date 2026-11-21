// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use serde_json::{Value, json};

use super::{TestResult, build, fixture, text};

#[test]
fn a_pool_lists_its_entries_and_the_places_that_reward_it() -> TestResult {
    let (_, catalog) = build()?;
    assert!(
        catalog.get("blueprintPools").is_none(),
        "no duplicated legacy pool root"
    );
    let pools = table_rows(&catalog, "/definitions/blueprintPools/rows")?;
    let pool = physical_pool(pools, fixture::POOL, "/guid/value")?;
    let weights = table_rows(&catalog, "/projections/blueprintPoolWeights/rows")?;
    let weight = physical_pool(weights, fixture::POOL, "/pool/guid/value")?;
    assert_eq!(
        weight.pointer("/totalWeight/value/finite"),
        Some(&json!(4.0))
    );
    assert_eq!(
        weight.pointer("/pool/recordIndex/value"),
        pool.pointer("/recordIndex/value")
    );
    let slots = table_rows(pool, "/blueprintRewards/value")?;
    let [first, second] = slots else {
        return Err("both authored pool slots".into());
    };
    assert_pool_slot(first, fixture::TWO_TIERS, 1.0);
    assert_pool_slot(second, fixture::CHOOSE_ONE, 3.0);
    let rewards = table_rows(&catalog, "/declarations/blueprintRewardSources/rows")?;
    let source = physical_pool(rewards, fixture::POOL, "/pool/value/guid/value")?;
    assert_reward_source(source, pool);
    let bad = physical_pool(weights, fixture::BAD_POOL, "/pool/guid/value")?;
    assert_eq!(
        bad.pointer("/totalWeight/value/finite"),
        Some(&json!(2.0)),
        "only positive weights count"
    );
    Ok(())
}

fn assert_reward_source(source: &Value, pool: &Value) {
    assert_eq!(
        source.pointer("/guid/value"),
        Some(&json!(text(fixture::CONTRACTS))),
        "the referencing Record retains its GUID"
    );
    assert_eq!(
        source.pointer("/pool/value/targetRecordIndex/value"),
        pool.pointer("/recordIndex/value"),
        "the link identifies the actual physical pool"
    );
    assert_eq!(
        source.pointer("/recordPath/value"),
        Some(&json!(
            "libs/foundry/records/contracts/contractgenerator/haul"
        )),
        "the referencing Record retains its path"
    );
    assert_eq!(
        source.pointer("/contractId/value"),
        Some(&json!(text(fixture::CONTRACT_ID))),
        "the enclosing contract supplies the GUID"
    );
    assert_eq!(
        source.pointer("/contractName/value"),
        Some(&json!("Haul_Intro")),
        "the same contract supplies the authored name"
    );
    assert_eq!(
        source.pointer("/chance/value/finite"),
        Some(&json!(0.5)),
        "the chance is retained at its original occurrence"
    );
    assert_eq!(
        source.pointer("/poolStorage/value"),
        Some(&json!("reference")),
        "direct Reference storage"
    );
    assert!(
        source
            .pointer("/sourceOrdinal/value")
            .and_then(Value::as_str)
            .is_some(),
        "physical source occurrence"
    );
}

fn table_rows<'value>(
    value: &'value Value,
    pointer: &str,
) -> Result<&'value [Value], Box<dyn Error>> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("required rows at {pointer}").into())
}

fn physical_pool<'rows>(
    rows: &'rows [Value],
    guid: u128,
    pointer: &str,
) -> Result<&'rows Value, Box<dyn Error>> {
    let expected = text(guid);
    rows.iter()
        .find(|row| row.pointer(pointer).and_then(Value::as_str) == Some(expected.as_str()))
        .ok_or_else(|| format!("expected pool {expected} at {pointer}").into())
}

fn assert_pool_slot(slot: &Value, guid: u128, weight: f32) {
    assert_eq!(
        slot.pointer("/value/blueprintRecord/value/guid/value"),
        Some(&json!(text(guid))),
        "the slot retains its authored Blueprint link"
    );
    assert_eq!(
        slot.pointer("/value/weight/value/rawBits"),
        Some(&json!(format!("{:08x}", weight.to_bits()))),
        "the stored IEEE 754 weight bits survive"
    );
    assert_eq!(
        slot.pointer("/value/weight/value/finite"),
        Some(&json!(weight)),
        "the finite weight has its numeric projection"
    );
}
