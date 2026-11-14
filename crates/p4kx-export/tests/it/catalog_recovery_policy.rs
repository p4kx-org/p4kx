// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::catalog_tags_fixture as tags;
use crate::health_origin_assertions as proof;
use crate::insurance_fixture::{RECOVERY, RECOVERY_PATH, build_from, diagnostic_from};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const ROOT: &str = "ItemRecoveryConfigurationParams";
const BASE: &str = "ItemRecoverySetConditionDef";
const CLASS: &str = "ItemRecoveryCondition_EntityClass";
const TAG: &str = "ItemRecoveryCondition_EntityTag";
const ITEM_TYPE: &str = "ItemRecoveryCondition_ItemType";
const FUTURE: &str = "ItemRecoveryCondition_Future";
const NOTIFICATION: &str = "ItemRecoveryNotificationParams";
const BUFFER: &str = "onBrickedItemUseNotificationBuffer";
const CONDITION_LISTS: &[&str] = &[
    "Insurable",
    "ReplenishOnRearm",
    "ReplenishOnClaimAndRepair",
    "NeverRepair",
    "NeverLTP",
    "ForceLTP",
    "DontEquipWhenBricked",
];
const NOTIFICATIONS: &[&str] = &[
    "itemBricked_Title",
    "itemBricked_Body",
    "vehicleBrickedWithItems_Body",
    "itemBricking_Title",
    "itemBricking_Body",
    "vehicleBrickingWithItems_Body",
    "onBrickedItemUsed_Title",
    "onBrickedItemUsed_CannotEquip",
    "onBrickedItemUsed_CannotFire",
    "onBrickedItemUsed_CannotUse",
];
const ENTITY: u128 = 0x2468;

#[test]
fn recovery_policy_retains_all_authored_conditions_and_protected_notifications() -> TestResult {
    let (_, catalog) = build_from(&fixture(false), None)?;
    assert_eq!(
        catalog["exportHealth"]["status"], "complete",
        "known source siblings publish"
    );
    let policy = policy(&catalog)?;
    let economy = proof::at(
        &catalog,
        "/configurations/itemRecoveryEconomy/rows/0/source",
    )?;
    assert_eq!(policy["ownerGuid"], economy["guid"], "same actual Record");
    assert_eq!(
        policy["ownerRecordIndex"], economy["recordIndex"],
        "same physical position"
    );
    check_conditions(&catalog, policy)?;
    check_notifications(&catalog, policy)?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&catalog),
        "schema admits original policy siblings"
    );
    Ok(())
}

#[test]
fn unknown_recovery_condition_is_diagnosed_without_losing_other_policy_values() -> TestResult {
    let forge = fixture(true);
    assert!(
        build_from(&forge, None).is_err(),
        "unknown populated branch prevents normal output"
    );
    let (_, catalog) = diagnostic_from(&forge, None)?;
    let policy = policy(&catalog)?;
    let original = &policy["NeverRepair"]["value"][0];
    assert_eq!(original["state"], "unsupported", "unknown concrete source");
    assert_eq!(
        original["containsEncryptedSource"], true,
        "actual source marker"
    );
    assert_eq!(
        catalog["exportHealth"]["status"], "incomplete",
        "projection loss remains visible"
    );
    check_notifications(&catalog, policy)
}

fn policy(catalog: &Value) -> TestResult<&Value> {
    let rows = proof::rows(catalog, "/configurations/itemRecoveryPolicy/rows")?;
    let [row] = rows else {
        return Err("one physical policy".into());
    };
    Ok(row)
}

fn check_conditions(catalog: &Value, policy: &Value) -> TestResult {
    for name in CONDITION_LISTS {
        proof::cell(policy, name, "present")?;
    }
    let entries = proof::rows(policy, "/Insurable/value")?;
    let [class, null, tag, item, base, repeated] = entries else {
        return Err("original condition slots and order".into());
    };
    assert_eq!(
        proof::at(null, "/state")?,
        "null",
        "explicit null condition slot"
    );
    check_class_condition(catalog, class)?;
    check_tag_condition(catalog, tag)?;
    let payload = proof::at(item, "/value/payload")?;
    proof::present(payload, "type", &json!("FixturePrimary"), Some(true))?;
    proof::present(payload, "subType", &json!("FixtureSubtype"), Some(true))?;
    assert_eq!(
        proof::at(base, "/value/payload/kind")?,
        "base",
        "fieldless base condition keeps its own discriminator"
    );
    assert_source(
        catalog,
        proof::at(repeated, "/value/payload/classDef/value/guid")?,
        "Insurable[5].classDef",
    )
}

fn check_class_condition(catalog: &Value, class: &Value) -> TestResult {
    assert_eq!(
        proof::at(class, "/value/sourceType/value")?,
        CLASS,
        "class condition keeps its original concrete type"
    );
    let payload = proof::at(class, "/value/payload")?;
    assert_eq!(
        proof::at(payload, "/kind")?,
        "entityClass",
        "entity class has its own discriminator"
    );
    let target = proof::at(payload, "/classDef/value")?;
    let guid = proof::present(target, "guid", &json!(tags::guid(ENTITY)), Some(true))?;
    assert_source(catalog, guid, "Insurable[0].classDef")
}

fn check_tag_condition(catalog: &Value, tag: &Value) -> TestResult {
    assert_eq!(
        proof::at(tag, "/value/sourceType/value")?,
        TAG,
        "tag condition keeps its original concrete type"
    );
    let tags = proof::rows(tag, "/value/payload/tags/value/tags/value")?;
    let [first, missing, second] = tags else {
        return Err("three original tag link slots".into());
    };
    assert_eq!(
        proof::at(missing, "/state")?,
        "null",
        "middle tag slot is authored null"
    );
    let first = proof::at(first, "/value/guid")?;
    let second = proof::at(second, "/value/guid")?;
    assert_eq!(
        proof::at(first, "/value")?,
        &json!(tags::guid(tags::ROOT)),
        "first authored tag resolves to the expected GUID"
    );
    assert_eq!(
        proof::at(first, "/value")?,
        proof::at(second, "/value")?,
        "the two authored tag slots retain the same target GUID"
    );
    assert_source(catalog, first, "Insurable[2].tags.tags[0]")?;
    assert_source(catalog, second, "Insurable[2].tags.tags[2]")
}

fn check_notifications(catalog: &Value, policy: &Value) -> TestResult {
    let notifications = proof::at(policy, "/notificationParams/value")?;
    let game_texts = proof::rows(catalog, "/texts/gameTexts")?;
    let public = proof::rows(catalog, "/texts/publicNames")?;
    for field in NOTIFICATIONS {
        let key = synthetic_key(field);
        let cell = proof::cell(notifications, field, "present")?;
        assert_eq!(cell["containsEncryptedSource"], true, "notification origin");
        assert_source(catalog, cell, &format!("notificationParams.{field}"))?;
        let reference = proof::at(cell, "/value")?;
        let text = game_texts
            .iter()
            .find(|text| text.pointer("/id/value") == Some(reference))
            .ok_or("canonical protected notification text")?;
        assert_eq!(
            proof::at(text, "/key/value")?,
            key.as_str(),
            "protected notification keeps its authored key {field}"
        );
        assert!(
            public.iter().all(|text| {
                text.pointer("/key/value").and_then(Value::as_str) != Some(key.as_str())
            }),
            "protected notification has no public text row"
        );
    }
    let buffer = proof::present(
        notifications,
        BUFFER,
        &json!({"rawBits":"7fc01080","finite":null}),
        Some(true),
    )?;
    assert_source(catalog, buffer, &format!("notificationParams.{BUFFER}"))
}

fn assert_source(catalog: &Value, cell: &Value, field: &str) -> TestResult {
    let uses = proof::uses(catalog, cell)?;
    assert!(
        uses.iter().any(|value| {
            value.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && value.get("role").and_then(Value::as_str) == Some("payload")
        }),
        "sourced value retains the exact original path {field}"
    );
    Ok(())
}

fn policy_schema(future: bool) -> DataForgeBuilder {
    let mut forge = tags::schema()
        .enumeration("FixtureType", ["FixturePrimary"])
        .enumeration("FixtureSubtype", ["FixtureSubtype"])
        .structure(TestStruct::new("EntityClassDefinition"))
        .structure(TestStruct::new(BASE))
        .structure(TestStruct::new(CLASS).parent(BASE).field(
            "classDef",
            TestType::ReferenceTo("EntityClassDefinition".to_owned()),
        ))
        .structure(
            TestStruct::new(TAG)
                .parent(BASE)
                .field("tags", TestType::Class("TagList".to_owned())),
        )
        .structure(
            TestStruct::new(ITEM_TYPE)
                .parent(BASE)
                .field("type", TestType::Enum("FixtureType".to_owned()))
                .field("subType", TestType::Enum("FixtureSubtype".to_owned())),
        )
        .structure(
            TestStruct::new("TagList").array("tags", TestType::ReferenceTo("Tag".to_owned())),
        )
        .structure(TestStruct::new("ItemRecoveryEconomyParams"));
    if future {
        forge = forge.structure(
            TestStruct::new(FUTURE)
                .parent(BASE)
                .field("flag", TestType::Bool),
        );
    }
    forge
}

fn fixture(future: bool) -> DataForgeBuilder {
    let (notice_shape, notices) = notifications();
    let mut root = TestStruct::new(ROOT)
        .field(
            "economyParams",
            TestType::Class("ItemRecoveryEconomyParams".to_owned()),
        )
        .field(
            "notificationParams",
            TestType::Class(NOTIFICATION.to_owned()),
        );
    let mut record = TestRecord::new(
        RECOVERY,
        ROOT,
        format!("{ROOT}.Fixture"),
        format!("{RECOVERY_PATH}.xml"),
    )
    .value(
        "economyParams",
        TestValue::Struct(TestInstance::new("ItemRecoveryEconomyParams")),
    )
    .value("notificationParams", TestValue::Struct(notices));
    for name in CONDITION_LISTS {
        root = root.array(*name, TestType::StrongPointer(BASE.to_owned()));
        record = record.value(*name, TestValue::Array(authored_conditions(name, future)));
    }
    policy_schema(future)
        .structure(notice_shape)
        .structure(root)
        .record(TestRecord::new(
            ENTITY,
            "EntityClassDefinition",
            "EntityClassDefinition.Fixture",
            "synthetic/entity.xml",
        ))
        .record(tags::tag(
            tags::ROOT,
            "Fixture",
            TestValue::Array(Vec::new()),
        ))
        .record(record)
}

fn authored_conditions(name: &str, future: bool) -> Vec<TestValue> {
    if name == "NeverRepair" && future {
        return vec![TestValue::Struct(
            TestInstance::new(FUTURE).value("flag", TestValue::Bool(true)),
        )];
    }
    if name != "Insurable" {
        return Vec::new();
    }
    let class = || {
        TestValue::Struct(TestInstance::new(CLASS).value("classDef", TestValue::Reference(ENTITY)))
    };
    let tags = TestInstance::new("TagList").value(
        "tags",
        TestValue::Array(vec![
            TestValue::Reference(tags::ROOT),
            TestValue::Null,
            TestValue::Reference(tags::ROOT),
        ]),
    );
    vec![
        class(),
        TestValue::Null,
        TestValue::Struct(TestInstance::new(TAG).value("tags", TestValue::Struct(tags))),
        TestValue::Struct(
            TestInstance::new(ITEM_TYPE)
                .value("type", TestValue::text("FixturePrimary"))
                .value("subType", TestValue::text("FixtureSubtype")),
        ),
        TestValue::Struct(TestInstance::new(BASE)),
        class(),
    ]
}

fn notifications() -> (TestStruct, TestInstance) {
    let mut shape = TestStruct::new(NOTIFICATION).field(BUFFER, TestType::Single);
    let mut value = TestInstance::new(NOTIFICATION)
        .value(BUFFER, TestValue::Single(f32::from_bits(0x7fc0_1080)));
    for name in NOTIFICATIONS {
        shape = shape.field(*name, TestType::Locale);
        value = value.value(*name, TestValue::text(synthetic_key(name)));
    }
    (shape, value)
}

fn synthetic_key(field: &str) -> String {
    format!("@fixture_recovery_{field}")
}
