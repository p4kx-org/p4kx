// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};
use serde_json::Value as Json;

use super::TestResult;

const MULTIPLE: &str = "MissionFlowAction_MultipleActions";
const RANDOM: &str = "MissionFlowAction_PickRandomAction";
const ENTRY: &str = "MissionRandomActionEntry";

#[derive(Default)]
pub(super) struct Counts {
    pub(super) multiple: usize,
    pub(super) random: usize,
}

pub(super) fn is_wrapper(original: &Value<'_>) -> bool {
    matches!(original, Value::Struct(source) if matches!(source.type_name(), MULTIPLE | RANDOM))
}

pub(super) fn check<'catalog>(
    source: &Value<'_>,
    root: &'catalog Json,
    values: &'catalog [Json],
    counts: &mut Counts,
) -> TestResult {
    let mut pending = vec![(source, root)];
    while let Some((source, node)) = pending.pop() {
        let Value::Struct(source) = source else {
            state(source, node)?;
            continue;
        };
        require(
            node.pointer("/value/value/sourceType/value")
                .and_then(Json::as_str)
                == Some(source.type_name()),
            "action retains its exact original concrete source type",
        )?;
        let tag = match source.type_name() {
            MULTIPLE => {
                counts.multiple += 1;
                multiple(source, node, values, &mut pending)?;
                "multipleActions"
            }
            RANDOM => {
                counts.random += 1;
                random(source, node, values, &mut pending)?;
                "pickRandomAction"
            }
            "MissionFlowAction_StateChange" => "stateChange",
            "MissionFlowAction_TokenStateChange" => "tokenStateChange",
            "MissionFlowAction_PickRandomMissionPhase" => "pickRandomMissionPhase",
            _ => "unsupported",
        };
        require(
            node.pointer("/action/type").and_then(Json::as_str) == Some(tag),
            "known nested action has its own typed meaning",
        )?;
    }
    Ok(())
}

fn multiple<'view, 'source, 'catalog>(
    source: &'view StructValue<'source>,
    node: &'catalog Json,
    values: &'catalog [Json],
    pending: &mut Vec<(&'view Value<'source>, &'catalog Json)>,
) -> TestResult {
    let actual = rows(node, "/action/actions")?;
    let original = original_slots(source);
    require(
        original.len() == actual.len(),
        "complete authored action array",
    )?;
    for (source, reference) in original.iter().zip(actual) {
        pending.push((source, follow(values, reference)?));
    }
    Ok(())
}

fn random<'view, 'source, 'catalog>(
    source: &'view StructValue<'source>,
    node: &'catalog Json,
    values: &'catalog [Json],
    pending: &mut Vec<(&'view Value<'source>, &'catalog Json)>,
) -> TestResult {
    let actual = rows(node, "/action/entries")?;
    let original = original_slots(source);
    require(
        original.len() == actual.len(),
        "complete authored weighted array",
    )?;
    for (source, entry) in original.iter().zip(actual) {
        let node = follow(values, entry.get("value").ok_or("original entry value")?)?;
        let Value::Struct(source) = source else {
            state(source, node)?;
            continue;
        };
        require(
            node.pointer("/value/value/sourceType/value")
                .and_then(Json::as_str)
                == Some(source.type_name()),
            "the actual weighted-entry source type",
        )?;
        if source.type_name() != ENTRY {
            require(
                entry["type"] == "unsupported",
                "unknown random entry stays explicit",
            )?;
            continue;
        }
        require(
            entry["type"] == "declared",
            "typed known random action entry",
        )?;
        check_probability(source, node, entry, values)?;
        match source.field("action") {
            Some(source) => pending.push((source, follow(values, &entry["action"])?)),
            None => require(
                entry["action"].is_null()
                    && entry.pointer("/actionField/type").and_then(Json::as_str) == Some("absent"),
                "missing authored action declaration stays absent",
            )?,
        }
    }
    Ok(())
}

fn check_probability(
    source: &StructValue<'_>,
    node: &Json,
    entry: &Json,
    values: &[Json],
) -> TestResult {
    let binding = &entry["probabilityField"];
    let Some(original) = source.field("probability") else {
        return require(
            binding["type"] == "absent",
            "original probability declaration absent",
        );
    };
    let index = decimal(binding, "fieldIndex")?;
    let field = rows(node, "/value/value/fields")?
        .get(index)
        .ok_or("original probability field")?;
    require(
        field.pointer("/name/value").and_then(Json::as_str) == Some("probability"),
        "binding selects the authored probability field",
    )?;
    let value = follow(values, &field["value"])?;
    match original {
        Value::Single(original) => require(
            value
                .pointer("/value/value/value/type")
                .and_then(Json::as_str)
                == Some("f32")
                && value
                    .pointer("/value/value/value/rawBits")
                    .and_then(Json::as_str)
                    == Some(format!("{:08x}", original.to_bits()).as_str()),
            "weighted action retains exact authored Single bits",
        ),
        _ => state(original, value),
    }
}

fn original_slots<'view, 'source>(source: &'view StructValue<'source>) -> &'view [Value<'source>] {
    match source.field("actions") {
        Some(Value::Array(values)) => values,
        _ => &[],
    }
}

fn state(source: &Value<'_>, node: &Json) -> TestResult {
    let expected = match source {
        Value::Null => "null",
        Value::Error(_) => "unreadable",
        _ => return Err("unsupported original action slot in private oracle".into()),
    };
    require(
        node.pointer("/value/state").and_then(Json::as_str) == Some(expected),
        "actual original action slot source state",
    )
}

pub(super) fn rows<'a>(node: &'a Json, path: &str) -> TestResult<&'a [Json]> {
    node.pointer(path)
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| "required action graph array".into())
}

pub(super) fn follow<'a>(values: &'a [Json], reference: &Json) -> TestResult<&'a Json> {
    values
        .get(decimal(reference, "valueIndex")?)
        .ok_or_else(|| "action value reference stays in its physical owner".into())
}

fn decimal(value: &Json, name: &str) -> TestResult<usize> {
    Ok(value[name]
        .as_str()
        .ok_or("canonical decimal control")?
        .parse()?)
}

fn require(passed: bool, detail: &'static str) -> TestResult {
    if passed { Ok(()) } else { Err(detail.into()) }
}
