// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};
use serde_json::Value as Json;

use super::TestResult;
use super::policy::{self, member, require, text};

const LEGACY: &[&str] = &[
    "globalBrickTimer",
    "baseReclaimTime",
    "baseDeliveryTime",
    "scalingPriceFloor",
    "aUECperSecond",
    "defaultLoadoutCooldownMultiplier",
    "defaultLoadoutDeliveryTimeMultiplier",
    "claimCostMultiplier",
    "vehicleChassisRepairCostMultiplier",
    "vehicleComponentRepairCostMultiplier",
    "vehicleRepairMissingItemCostMultiplier",
    "deliverySecondsPerAUEC",
    "expeditedDeliverySecondsPerAUEC",
    "defaultLoadoutExpeditedDeliveryTimeMultiplier",
    "flatMinimumDeliveryTimerSeconds",
    "maxExpeditedClaimCostMultiplier",
];
const ESCALATING: &[&str] = &[
    "escalatingClaimChainLimit",
    "escalationTimerMultiplier",
    "escalatingClaimCostMultiplier",
    "escalatingDeliveryTimerMultiplier",
];
const OVERRIDES: &[&str] = &[
    "claimCooldownMultiplier",
    "claimCostMultiplier",
    "deliveryTimeMultiplier",
    "maxExpediteCostMultiplier",
    "expeditedDeliveryTimeMultiplier",
    "flatMinimumDeliveryTimerSecondsMultiplier",
    "escalationTimerMultiplier",
    "escalatingClaimCostMultiplier",
    "escalatingDeliveryTimerMultiplier",
];

pub(super) fn check(
    original: &StructValue<'_>,
    economy: &Json,
    catalog: &Json,
    owner: u32,
) -> TestResult {
    numbers(original, member(economy, "legacy")?, LEGACY)?;
    let escalation = member(economy, "escalatingClaimsParams")?;
    match original.field("escalatingClaimsParams") {
        Some(Value::Struct(source)) => {
            require(
                text(escalation, "/state") == Some("present"),
                "authored escalation block",
            )?;
            numbers(source, member(escalation, "value")?, ESCALATING)?;
        }
        other => policy::state(other, escalation)?,
    }
    let rows = catalog
        .pointer("/configurations/itemRecoveryOverrides/rows")
        .and_then(Json::as_array)
        .ok_or("canonical recovery overrides")?;
    let [row] = rows.as_slice() else {
        return Err("exactly one override owner".into());
    };
    require(
        row.pointer("/ownerRecordIndex/value")
            .and_then(Json::as_u64)
            == Some(u64::from(owner)),
        "same physical policy for the override array",
    )?;
    policy::check_array(
        original.field("overrides"),
        member(row, "overrides")?,
        check_override,
    )
}

fn check_override(source: &Value<'_>, cell: &Json) -> TestResult {
    let Value::Struct(group) = source else {
        return policy::state(Some(source), cell);
    };
    require(
        text(cell, "/state") == Some("present"),
        "authored override group",
    )?;
    let target = member(cell, "value")?;
    numbers(group, target, OVERRIDES)?;
    policy::check_array(
        group.field("classes"),
        member(target, "classes")?,
        |source, target| policy::check_reference(Some(source), target),
    )
}

fn numbers(source: &StructValue<'_>, projected: &Json, names: &[&str]) -> TestResult {
    for name in names {
        let cell = member(projected, name)?;
        match source.field(name) {
            Some(
                value @ (Value::Int8(_)
                | Value::Int16(_)
                | Value::Int32(_)
                | Value::Int64(_)
                | Value::UInt8(_)
                | Value::UInt16(_)
                | Value::UInt32(_)
                | Value::UInt64(_)
                | Value::Single(_)
                | Value::Double(_)),
            ) => {
                require(
                    text(cell, "/state") == Some("present"),
                    "original legacy numeric value",
                )?;
                require(
                    same_number(value, member(cell, "value")?),
                    "legacy or override scalar differs from the original storage",
                )?;
            }
            other => policy::state(other, cell)?,
        }
    }
    Ok(())
}

fn same_number(source: &Value<'_>, target: &Json) -> bool {
    let word = target.get("value");
    let shape = text(target, "/type");
    match source {
        Value::Single(value) => {
            shape == Some("f32")
                && text(target, "/rawBits") == Some(format!("{:08x}", value.to_bits()).as_str())
        }
        Value::Double(value) => {
            shape == Some("f64")
                && text(target, "/rawBits") == Some(format!("{:016x}", value.to_bits()).as_str())
        }
        Value::Int8(value) => {
            shape == Some("int8") && word.and_then(Json::as_i64) == Some(i64::from(*value))
        }
        Value::Int16(value) => {
            shape == Some("int16") && word.and_then(Json::as_i64) == Some(i64::from(*value))
        }
        Value::Int32(value) => {
            shape == Some("int32") && word.and_then(Json::as_i64) == Some(i64::from(*value))
        }
        Value::Int64(value) => {
            shape == Some("int64")
                && word.and_then(Json::as_str) == Some(value.to_string().as_str())
        }
        Value::UInt8(value) => {
            shape == Some("uint8") && word.and_then(Json::as_u64) == Some(u64::from(*value))
        }
        Value::UInt16(value) => {
            shape == Some("uint16") && word.and_then(Json::as_u64) == Some(u64::from(*value))
        }
        Value::UInt32(value) => {
            shape == Some("uint32") && word.and_then(Json::as_u64) == Some(u64::from(*value))
        }
        Value::UInt64(value) => {
            shape == Some("uint64")
                && word.and_then(Json::as_str) == Some(value.to_string().as_str())
        }
        _ => false,
    }
}
