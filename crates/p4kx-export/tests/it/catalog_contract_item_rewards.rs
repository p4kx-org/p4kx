// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

#[path = "contract_item_reward_fixture.rs"]
mod fixture;

#[test]
fn item_rewards_keep_occurrences_layers_and_independent_blueprint_results() -> TestResult {
    let data = fixture::database();
    let fixture = Fixture::open(&data, Input::Plain, Vec::new())?;
    let (_, catalog) = fixture.diagnostic()?;
    let definition = definition(&catalog)?;
    let [first, repeated, no_target, unresolved, whole] =
        origin::rows(definition, "/itemRewardResults")?
    else {
        return Err("all five physical item reward occurrences".into());
    };
    let parent = entry(&catalog, None)?;
    let fallback = entry(&catalog, Some(fixture::FALLBACK))?;
    let child = entry(&catalog, Some(fixture::SUB))?;
    let expected = [first, repeated, no_target, unresolved]
        .map(item_index)
        .into_iter()
        .collect::<TestResult<Vec<_>>>()?;
    assert_ne!(
        expected[0], expected[1],
        "identical results have separate slots"
    );
    assert_eq!(
        entry_links(parent)?,
        expected,
        "parent results keep physical order"
    );
    assert_eq!(
        entry_links(fallback)?,
        expected,
        "an empty child block uses the parent"
    );
    assert_eq!(
        entry_links(child)?,
        vec![item_index(whole)?],
        "owned child block wins"
    );
    assert_eq!(
        origin::rows(parent, "/blueprintRewards")?.len(),
        1,
        "independent Blueprint result"
    );
    assert_eq!(
        origin::rows(fallback, "/blueprintRewards")?.len(),
        1,
        "fallback Blueprint result"
    );
    assert!(
        origin::rows(child, "/blueprintRewards")?.is_empty(),
        "no Blueprint result in own block"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    validator
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    old_consumer(&catalog, expected[0])?;
    origin::closure(&catalog)
}

#[test]
fn item_reward_locations_keep_null_unreadable_indices_and_encryption_distinct() -> TestResult {
    let data = fixture::database();
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let fixture = Fixture::open(&data, input, Vec::new())?;
        let (_, catalog) = fixture.diagnostic()?;
        let definition = definition(&catalog)?;
        let [local, _, no_target, unresolved, whole] =
            origin::rows(definition, "/itemRewardResults")?
        else {
            return Err("all original item reward rows".into());
        };
        verify_local(&catalog, definition, local, input)?;
        verify_location_states(no_target, unresolved, whole, input)?;
        origin::closure(&catalog)?;
    }
    Ok(())
}

fn definition(catalog: &Value) -> TestResult<&Value> {
    let [row] = origin::rows(catalog, "/definitions/contractGenerators/rows")? else {
        return Err("one original ContractGenerator".into());
    };
    Ok(row)
}

fn entry(catalog: &Value, sub: Option<u128>) -> TestResult<&Value> {
    let parent = health_origin_fixture::guid(fixture::PARENT);
    let sub = sub.map(health_origin_fixture::guid);
    catalog_contracts::entry(catalog, &parent, sub.as_deref())
}

fn item_index(reward: &Value) -> TestResult<&Value> {
    origin::at(reward, "/source/valueIndex")
}

fn entry_links(entry: &Value) -> TestResult<Vec<&Value>> {
    origin::rows(entry, "/itemRewardResults")?
        .iter()
        .map(|link| origin::at(link, "/value/valueIndex"))
        .collect()
}

fn raw<'a>(definition: &'a Value, index: &Value) -> TestResult<&'a Value> {
    let index = index.as_str().ok_or("source value index text")?;
    origin::at(definition, &format!("/source/values/{index}/value/value"))
}

fn verify_local(catalog: &Value, definition: &Value, reward: &Value, input: Input) -> TestResult {
    let amount = origin::present(reward, "amount", &json!("-4"), input.marker())?;
    assert!(
        origin::uses(catalog, amount)?.iter().any(|used| {
            used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
                && used.pointer("/location/field") == Some(&json!("$value"))
                && used.get("role") == Some(&json!("payload"))
        }),
        "quantity retains a physical original value use"
    );
    origin::present(
        reward,
        "awardOnlyToMissionOwner",
        &json!(true),
        input.marker(),
    )?;
    origin::present(
        reward,
        "sendToPlayerHomeLocation",
        &json!(false),
        input.marker(),
    )?;
    let outcome = origin::rows(reward, "/missionResults/value")?;
    let values = outcome
        .iter()
        .map(|flag| origin::at(flag, "/value"))
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(
        values,
        [&json!(true), &json!(false), &json!(true)],
        "stored outcomes in order"
    );
    let reference = origin::cell(reward, "entityClass", "present")?;
    assert_eq!(
        origin::at(reference, "/value/targetRecordIndex/value")?,
        1,
        "physical entity index"
    );
    verify_target_selector(catalog, definition, reward)?;
    verify_local_arena_links(definition, reward, input)
}

fn verify_target_selector(catalog: &Value, definition: &Value, reward: &Value) -> TestResult {
    let source = raw(definition, item_index(reward)?)?;
    let declaration = origin::rows(source, "/fields")?
        .iter()
        .find(|field| {
            field.pointer("/name/value").and_then(Value::as_str) == Some("targetLocation")
        })
        .ok_or("the physical targetLocation field in the original reward row")?;
    let record = origin::at(definition, "/recordIndex/value")?
        .as_u64()
        .ok_or("the physical Generator Record index")?;
    let indices = json!([
        record.to_string(),
        origin::at(declaration, "/value/valueIndex")?
    ]);
    let target = origin::cell(reward, "targetLocation", "present")?;
    let uses = origin::uses(catalog, target)?;
    assert_eq!(
        uses.len(),
        3,
        "the typed selector retains only its authored field inputs"
    );
    for field in ["$value", "$fieldName", "$dataType"] {
        let location = json!({
            "type": "subresource",
            "namespace": "dataforgeValue",
            "indices": indices,
            "field": field
        });
        assert!(
            uses.iter().any(|used| {
                used.get("location") == Some(&location)
                    && used.get("role").and_then(Value::as_str) == Some("selector")
            }),
            "the target's {field} selects its exact original occurrence"
        );
    }
    Ok(())
}

fn verify_local_arena_links(definition: &Value, reward: &Value, input: Input) -> TestResult {
    let target = origin::cell(reward, "targetLocation", "present")?;
    assert_eq!(
        origin::at(target, "/containsEncryptedSource")?,
        &json!(input.marker()),
        "selector encryption"
    );
    assert_eq!(
        origin::at(target, "/value/reference/target/type")?,
        "instance",
        "local weak target"
    );
    assert_eq!(
        origin::at(target, "/value/reference/target/location/value")?,
        "locations[1]",
        "the authored array slot, not the similar first property"
    );
    let joined = raw(
        definition,
        origin::at(target, "/value/targetValue/valueIndex")?,
    )?;
    assert_eq!(
        origin::at(joined, "/sourceType/value")?,
        "MissionProperty",
        "exact target is in the canonical graph"
    );
    let note = raw(
        definition,
        origin::at(reward, "/notification/value/valueIndex")?,
    )?;
    assert_eq!(
        origin::at(note, "/sourceType/value")?,
        "FixtureNotification",
        "structured notification remains linked"
    );
    Ok(())
}

fn verify_location_states(
    no_target: &Value,
    unresolved: &Value,
    whole: &Value,
    input: Input,
) -> TestResult {
    let null = origin::cell(no_target, "targetLocation", "null")?;
    assert_eq!(
        origin::at(null, "/containsEncryptedSource")?,
        &json!(input.marker()),
        "null field's origin"
    );
    let failed = origin::cell(unresolved, "targetLocation", "unreadable")?;
    assert_eq!(
        origin::at(failed, "/sourceSetComplete")?,
        false,
        "unresolved weak selector"
    );
    let target = origin::cell(whole, "targetLocation", "present")?;
    assert_eq!(
        origin::at(target, "/value/reference/targetRecordIndex/value")?,
        0,
        "record index zero is an identity"
    );
    assert_eq!(
        origin::at(target, "/value/reference/target/type")?,
        "record",
        "typed whole Record"
    );
    assert_eq!(
        origin::at(target, "/value/targetValue")?,
        &Value::Null,
        "whole Record has no arena-local target"
    );
    origin::present(whole, "amount", &json!("0"), input.marker())?;
    origin::present(
        whole,
        "sendToPlayerHomeLocation",
        &json!(true),
        input.marker(),
    )?;
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlueprintRewardConsumer {
    blueprint_rewards: Vec<Value>,
}

fn old_consumer(catalog: &Value, first: &Value) -> TestResult {
    let parent = entry(catalog, None)?;
    let result: BlueprintRewardConsumer = serde_json::from_value(parent.clone())?;
    assert_eq!(
        result.blueprint_rewards,
        origin::rows(parent, "/blueprintRewards")?,
        "a v3 consumer reading its existing fields ignores the additive reward references"
    );
    assert_eq!(
        entry_links(parent)?.first().copied(),
        Some(first),
        "item result remains independently addressable"
    );
    Ok(())
}
