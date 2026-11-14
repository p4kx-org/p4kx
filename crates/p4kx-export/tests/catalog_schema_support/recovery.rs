// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;
use serde_json::{Map, Value, json};

use super::source_values::{present, source};

const GUID: &str = "c4ac0f5a-c072-4666-b3a0-5aef0cf3ff65";
const PATH: &str = "libs/foundry/records/entitlementpolicies/synthetic-recovery";
const ROOT: &str = "ItemRecoveryConfigurationParams";

pub(super) fn economy() -> Value {
    json!({
        "availability": "available",
        "rows": [{
            "source": {
                "guid": present(json!(GUID)),
                "recordName": present(json!("ItemRecoveryConfigurationParams.Synthetic")),
                "className": present(json!("Synthetic")),
                "recordPath": present(json!(PATH)),
                "recordIndex": present(json!(0)),
                "sourceType": present(json!(ROOT)),
                "fileName": present(json!(format!("{PATH}.xml"))),
                "metadataTag": source(SourceValue::Absent)
            },
            "economyParams": present(economy_parameters())
        }]
    })
}

fn economy_parameters() -> Value {
    let legacy = fields(
        &[
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
        ],
        legacy_number,
    );
    let cooldown = fields(
        &[
            "baseTime",
            "scalingPriceFloor",
            "aUECperSecond",
            "defaultLoadoutMultiplier",
        ],
        single,
    );
    let delivery = fields(
        &[
            "baseTime",
            "scalingPriceFloor",
            "aUECperSecond",
            "defaultLoadoutMultiplier",
            "expeditedTimeMultiplier",
            "expeditedDefaultTimeMultiplier",
        ],
        single,
    );
    let repair = fields(
        &[
            "vehicleChassisRepairCostMultiplier",
            "vehicleComponentRepairCostMultiplier",
            "vehicleRepairMissingItemCostMultiplier",
        ],
        single,
    );
    json!({
        "legacy": legacy,
        "cooldownParams": present(json!(cooldown)),
        "deliveryParams": present(json!(delivery)),
        "repairParams": present(json!(repair)),
        "escalatingClaimsParams": source(SourceValue::Absent)
    })
}

pub(super) fn overrides() -> Value {
    let mut group = fields(
        &[
            "claimCooldownMultiplier",
            "claimCostMultiplier",
            "deliveryTimeMultiplier",
            "maxExpediteCostMultiplier",
            "expeditedDeliveryTimeMultiplier",
            "flatMinimumDeliveryTimerSecondsMultiplier",
            "escalationTimerMultiplier",
            "escalatingClaimCostMultiplier",
            "escalatingDeliveryTimerMultiplier",
        ],
        legacy_number,
    );
    group.insert("classes".to_owned(), present(json!([])));
    json!({
        "availability": "available",
        "rows": [{
            "ownerGuid": present(json!(GUID)),
            "ownerRecordIndex": present(json!(0)),
            "overrides": present(json!([present(json!(group))]))
        }]
    })
}

pub(super) fn policy() -> Value {
    json!({
        "availability": "available",
        "rows": [{
            "ownerGuid": present(json!(GUID)),
            "ownerRecordIndex": present(json!(0)),
            "Insurable": present(json!([present(json!({
                "sourceType": present(json!("ItemRecoverySetConditionDef")),
                "payload": {"kind": "base"}
            }))])),
            "ReplenishOnRearm": source(SourceValue::Absent),
            "ReplenishOnClaimAndRepair": source(SourceValue::Absent),
            "NeverRepair": source(SourceValue::Absent),
            "NeverLTP": source(SourceValue::Absent),
            "ForceLTP": source(SourceValue::Absent),
            "DontEquipWhenBricked": source(SourceValue::Absent),
            "notificationParams": source(SourceValue::Absent)
        }]
    })
}

fn fields(names: &[&str], cell: fn() -> Value) -> Map<String, Value> {
    names
        .iter()
        .map(|name| ((*name).to_owned(), cell()))
        .collect()
}

fn single() -> Value {
    present(json!({"rawBits": "00000000", "finite": 0.0}))
}

fn legacy_number() -> Value {
    present(json!({"type": "f32", "rawBits": "00000000", "finite": 0.0}))
}
