// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use crate::health_origin_assertions as proof;

use super::{economy, legacy, overrides};
use crate::insurance_fixture::{RECOVERY, RECOVERY_PATH, build_from, diagnostic_from};

type TestResult = Result<(), Box<dyn Error>>;

const ROOT: &str = "ItemRecoveryConfigurationParams";
const ECONOMY: &str = "ItemRecoveryEconomyParams";
const OVERRIDE: &str = "ItemRecoveryOverrideGroupDef";
const REMOVED: &str = "flatMinimumDeliveryTimerSecondsMultiplier";

const LEGACY_FIELDS: &[&str] = &[
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
const COOLDOWN_FIELDS: &[&str] = &[
    "baseTime",
    "scalingPriceFloor",
    "aUECperSecond",
    "defaultLoadoutMultiplier",
];
const DELIVERY_FIELDS: &[&str] = &[
    "baseTime",
    "scalingPriceFloor",
    "aUECperSecond",
    "defaultLoadoutMultiplier",
    "expeditedTimeMultiplier",
    "expeditedDefaultTimeMultiplier",
];
const REPAIR_FIELDS: &[&str] = &[
    "vehicleChassisRepairCostMultiplier",
    "vehicleComponentRepairCostMultiplier",
    "vehicleRepairMissingItemCostMultiplier",
];
const OVERRIDE_FIELDS: &[&str] = &[
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

const GROUPS: [Group; 3] = [
    Group {
        name: "cooldownParams",
        source: "ItemRecoveryCooldownParams",
        fields: COOLDOWN_FIELDS,
        start: 0,
    },
    Group {
        name: "deliveryParams",
        source: "ItemRecoveryDeliveryTimeParams",
        fields: DELIVERY_FIELDS,
        start: 32,
    },
    Group {
        name: "repairParams",
        source: "ItemRecoveryRepairParams",
        fields: REPAIR_FIELDS,
        start: 64,
    },
];

struct Group {
    name: &'static str,
    source: &'static str,
    fields: &'static [&'static str],
    start: u16,
}

#[test]
fn nested_and_legacy_item_recovery_keep_distinct_values_and_original_coordinates() -> TestResult {
    for (has_legacy, has_nested) in [(true, false), (false, true), (true, true)] {
        let (_, catalog) = build_from(&fixture(has_legacy, has_nested), None)?;
        assert_eq!(catalog["schemaVersion"], 3);
        assert!(
            catalog.get("itemRecovery").is_none(),
            "canonical v3 has a single recovery wire view"
        );
        check_legacy(&catalog, has_legacy)?;
        check_nested(&catalog, has_nested)?;
        check_overrides(&catalog, has_legacy)?;
        let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
        assert!(
            validator.is_valid(&catalog),
            "current schema admits the real writer output"
        );
    }
    Ok(())
}

#[test]
fn malformed_and_unsupported_nested_blocks_remain_distinct() -> TestResult {
    for malformed in [true, false] {
        let fixture = malformed_fixture(malformed);
        assert!(
            build_from(&fixture, None).is_err(),
            "known incomplete projection blocks normal publication"
        );
        let (_, catalog) = diagnostic_from(&fixture, None)?;
        let expected = if malformed { "invalid" } else { "unsupported" };
        let economy = economy(&catalog)?;
        assert_eq!(economy["cooldownParams"]["state"], expected);
        assert_eq!(economy["deliveryParams"]["state"], "absent");
        assert_eq!(economy["repairParams"]["state"], "absent");
    }
    Ok(())
}

#[test]
fn explicit_null_and_unreadable_recovery_block_keep_independent_states() -> TestResult {
    const COOLDOWN: &str = "ItemRecoveryCooldownParams";
    let bad_pointer = [u32::MAX.to_le_bytes(), 0_u32.to_le_bytes()].concat();
    for (value, expected) in [
        (TestValue::Null, "null"),
        (TestValue::Raw(bad_pointer), "unreadable"),
    ] {
        let source = DataForgeBuilder::new()
            .version(8)
            .structure(TestStruct::new(COOLDOWN));
        let shape = TestStruct::new(ECONOMY).field(
            "cooldownParams",
            TestType::StrongPointer(COOLDOWN.to_owned()),
        );
        let value = TestInstance::new(ECONOMY).value("cooldownParams", value);
        let fixture = finish(source, shape, value);
        let (_, catalog) = if expected == "null" {
            build_from(&fixture, None)?
        } else {
            assert!(
                build_from(&fixture, None).is_err(),
                "unreadable original input prevents normal output"
            );
            diagnostic_from(&fixture, None)?
        };
        let fields = economy(&catalog)?;
        assert_eq!(
            fields["cooldownParams"]["state"], expected,
            "authored pointer state"
        );
        assert_eq!(fields["cooldownParams"]["value"], Value::Null);
        assert_eq!(fields["repairParams"]["state"], "absent");
    }
    Ok(())
}

fn check_legacy(catalog: &Value, present: bool) -> TestResult {
    let old = legacy(catalog)?;
    for (offset, name) in LEGACY_FIELDS.iter().enumerate() {
        let cell = old
            .get(*name)
            .ok_or_else(|| format!("missing legacy recovery field {name}"))?;
        if present {
            scalar(
                catalog,
                cell,
                &format!("economyParams.{name}"),
                f32::from(128 + u16::try_from(offset)?),
            )?;
            assert_eq!(
                proof::at(cell, "/value/type")?,
                "f32",
                "original source width"
            );
        } else {
            assert_eq!(
                proof::at(cell, "/state")?,
                "absent",
                "new source does not backfill {name}"
            );
        }
    }
    Ok(())
}

fn check_nested(catalog: &Value, present: bool) -> TestResult {
    let economy = economy(catalog)?;
    for group in GROUPS {
        let block = economy
            .get(group.name)
            .ok_or_else(|| format!("missing original recovery block {}", group.name))?;
        if !present {
            assert_eq!(
                proof::at(block, "/state")?,
                "absent",
                "legacy source lacks {}",
                group.name
            );
            continue;
        }
        assert_eq!(
            proof::at(block, "/state")?,
            "present",
            "nested source block"
        );
        let fields = proof::at(block, "/value")?;
        for (ordinal, name) in group.fields.iter().enumerate() {
            let cell = fields
                .get(*name)
                .ok_or_else(|| format!("missing nested recovery field {}.{name}", group.name))?;
            scalar(
                catalog,
                cell,
                &format!("economyParams.{}.{name}", group.name),
                group_value(group.start, ordinal),
            )?;
        }
    }
    Ok(())
}

fn check_overrides(catalog: &Value, has_legacy: bool) -> TestResult {
    let value = proof::at(overrides(catalog)?, "/value/0/value")?;
    for (ordinal, name) in OVERRIDE_FIELDS.iter().enumerate() {
        let cell = value
            .get(*name)
            .ok_or_else(|| format!("missing original recovery override {name}"))?;
        if *name == REMOVED && !has_legacy {
            assert_eq!(
                proof::at(cell, "/state")?,
                "absent",
                "removed override has no invented substitute"
            );
            continue;
        }
        scalar(
            catalog,
            cell,
            &format!("economyParams.overrides[0].{name}"),
            f32::from(192 + u16::try_from(ordinal)?),
        )?;
    }
    Ok(())
}

fn scalar(catalog: &Value, cell: &Value, path: &str, expected: f32) -> TestResult {
    assert_eq!(proof::at(cell, "/state")?, "present", "{path}");
    assert_eq!(
        proof::at(cell, "/containsEncryptedSource")?.as_bool(),
        Some(true),
        "actual encrypted source for {path}"
    );
    assert_eq!(
        proof::at(cell, "/sourceSetComplete")?.as_bool(),
        Some(true),
        "complete original binding for {path}"
    );
    assert_eq!(
        proof::at(cell, "/value/rawBits")?,
        format!("{:08x}", expected.to_bits()).as_str(),
        "original IEEE bits of {path}"
    );
    if !expected.is_finite() {
        assert!(
            proof::at(cell, "/value/finite")?.is_null(),
            "legal nonfinite value has no decimal representation"
        );
    }
    assert!(
        proof::uses(catalog, cell)?.iter().any(|used| {
            used.pointer("/location/field").and_then(Value::as_str) == Some(path)
                && used.get("role").and_then(Value::as_str) == Some("payload")
        }),
        "exact original payload coordinate for {path}"
    );
    Ok(())
}

fn fixture(has_legacy: bool, has_nested: bool) -> DataForgeBuilder {
    let mut forge = DataForgeBuilder::new().version(8);
    let mut economy_type = TestStruct::new(ECONOMY);
    let mut economy = TestInstance::new(ECONOMY);
    if has_legacy {
        (economy_type, economy) = numbers(economy_type, economy, LEGACY_FIELDS, 128);
    }
    if has_nested {
        for group in GROUPS {
            let (shape, value) = numbers(
                TestStruct::new(group.source),
                TestInstance::new(group.source),
                group.fields,
                group.start,
            );
            forge = forge.structure(shape);
            economy_type = economy_type.field(group.name, TestType::Class(group.source.to_owned()));
            economy = economy.value(group.name, TestValue::Struct(value));
        }
    }
    let mut shape = TestStruct::new(OVERRIDE).array("classes", TestType::Reference);
    let mut group = TestInstance::new(OVERRIDE).value("classes", TestValue::Array(Vec::new()));
    for (ordinal, name) in OVERRIDE_FIELDS.iter().enumerate() {
        if has_legacy || *name != REMOVED {
            shape = shape.field(*name, TestType::Single);
            group = group.value(*name, TestValue::Single(group_value(192, ordinal)));
        }
    }
    forge = forge.structure(shape);
    economy_type = economy_type.array("overrides", TestType::Class(OVERRIDE.to_owned()));
    economy = economy.value(
        "overrides",
        TestValue::Array(vec![TestValue::Struct(group)]),
    );
    finish(forge, economy_type, economy)
}

fn malformed_fixture(malformed: bool) -> DataForgeBuilder {
    let mut forge = DataForgeBuilder::new().version(8);
    let (kind, value) = if malformed {
        (TestType::String, TestValue::text("wrong field type"))
    } else {
        let name = "UnknownRecoveryCooldown";
        forge = forge.structure(TestStruct::new(name).field("number", TestType::Single));
        (
            TestType::Class(name.to_owned()),
            TestValue::Struct(TestInstance::new(name).value("number", TestValue::Single(8.0))),
        )
    };
    finish(
        forge,
        TestStruct::new(ECONOMY).field("cooldownParams", kind),
        TestInstance::new(ECONOMY).value("cooldownParams", value),
    )
}

fn numbers(
    mut shape: TestStruct,
    mut value: TestInstance,
    fields: &[&str],
    start: u16,
) -> (TestStruct, TestInstance) {
    for (ordinal, name) in fields.iter().enumerate() {
        shape = shape.field(*name, TestType::Single);
        value = value.value(*name, TestValue::Single(group_value(start, ordinal)));
    }
    (shape, value)
}

fn group_value(start: u16, ordinal: usize) -> f32 {
    match (start, ordinal) {
        (0, 0) => -0.0,
        (64, 2) => f32::from_bits(0x7fc0_1080),
        _ => f32::from(start + u16::try_from(ordinal).unwrap_or_default()),
    }
}

fn finish(
    forge: DataForgeBuilder,
    economy_type: TestStruct,
    economy: TestInstance,
) -> DataForgeBuilder {
    forge
        .structure(economy_type)
        .structure(
            TestStruct::new(ROOT).field("economyParams", TestType::Class(ECONOMY.to_owned())),
        )
        .record(
            TestRecord::new(
                RECOVERY,
                ROOT,
                format!("{ROOT}.Synthetic"),
                format!("{RECOVERY_PATH}.xml"),
            )
            .value("economyParams", TestValue::Struct(economy)),
        )
}

#[path = "catalog_recovery_declared_targets.rs"]
mod declared_targets;
