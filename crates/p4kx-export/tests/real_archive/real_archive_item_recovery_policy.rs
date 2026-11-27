// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, StructValue, Value, ValueError};
use serde_json::Value as Json;

use super::TestResult;

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
const BUFFER: &str = "onBrickedItemUseNotificationBuffer";

pub(super) fn check(original: &RecordTree<'_>, catalog: &Json) -> TestResult {
    let rows = catalog
        .pointer("/configurations/itemRecoveryPolicy/rows")
        .and_then(Json::as_array)
        .ok_or("canonical recovery policy")?;
    let [row] = rows.as_slice() else {
        return Err("exactly one selected recovery policy".into());
    };
    require(
        row.pointer("/ownerRecordIndex/value")
            .and_then(Json::as_u64)
            == Some(u64::from(original.record_index())),
        "recovery policy has the correct physical owner",
    )?;
    for name in CONDITION_LISTS {
        check_array(original.field(name), member(row, name)?, check_condition)?;
    }
    let notifications = member(row, "notificationParams")?;
    match original.field("notificationParams") {
        Some(Value::Struct(notices)) => check_notices(notices, notifications, catalog),
        other => state(other, notifications),
    }
}

fn check_condition(original: &Value<'_>, cell: &Json) -> TestResult {
    let Value::Struct(condition) = original else {
        return state(Some(original), cell);
    };
    require(
        text(cell, "/state") == Some("present"),
        "readable original policy condition",
    )?;
    let value = member(cell, "value")?;
    let payload = member(value, "payload")?;
    require(
        text(value, "/sourceType/value") == Some(condition.type_name()),
        "original concrete condition type",
    )?;
    match condition.type_name() {
        "ItemRecoverySetConditionDef" => require(
            text(payload, "/kind") == Some("base"),
            "fieldless condition",
        ),
        "ItemRecoveryCondition_EntityClass" => {
            require(
                text(payload, "/kind") == Some("entityClass"),
                "entity-class condition",
            )?;
            check_reference(condition.field("classDef"), member(payload, "classDef")?)
        }
        "ItemRecoveryCondition_EntityTag" => {
            require(text(payload, "/kind") == Some("entityTag"), "tag condition")?;
            check_tags(condition.field("tags"), member(payload, "tags")?)
        }
        "ItemRecoveryCondition_ItemType" => {
            require(
                text(payload, "/kind") == Some("itemType"),
                "item-type condition",
            )?;
            check_enum(condition.field("type"), member(payload, "type")?)?;
            check_enum(condition.field("subType"), member(payload, "subType")?)
        }
        _ => Err("new original policy condition type requires review".into()),
    }
}

fn check_tags(original: Option<&Value<'_>>, cell: &Json) -> TestResult {
    let Some(Value::Struct(tags)) = original else {
        return state(original, cell);
    };
    require(
        text(cell, "/state") == Some("present"),
        "readable condition tag list",
    )?;
    check_array(
        tags.field("tags"),
        cell.pointer("/value/tags").ok_or("condition tag slots")?,
        |source, target| check_reference(Some(source), target),
    )
}

pub(super) fn check_array(
    original: Option<&Value<'_>>,
    cell: &Json,
    mut compare: impl FnMut(&Value<'_>, &Json) -> TestResult,
) -> TestResult {
    let Some(Value::Array(values)) = original else {
        return state(original, cell);
    };
    require(
        text(cell, "/state") == Some("present"),
        "original recovery array exists",
    )?;
    let exported = member(cell, "value")?
        .as_array()
        .ok_or("ordered recovery slots")?;
    require(
        exported.len() == values.len(),
        "every authored recovery slot",
    )?;
    for (source, target) in values.iter().zip(exported) {
        compare(source, target)?;
    }
    Ok(())
}

pub(super) fn check_reference(original: Option<&Value<'_>>, cell: &Json) -> TestResult {
    match original {
        Some(Value::Reference(target)) => require(
            text(cell, "/state") == Some("present")
                && text(cell, "/value/status") == Some("present")
                && text(cell, "/value/guid/value") == Some(target.guid().to_string().as_str())
                && cell
                    .pointer("/value/targetRecordIndex/value")
                    .and_then(Json::as_u64)
                    == Some(u64::from(target.record_index())),
            "exact physical recovery reference",
        ),
        Some(Value::Error(ValueError::DanglingReference { guid })) => require(
            text(cell, "/state") == Some("present")
                && text(cell, "/value/status") == Some("dangling")
                && text(cell, "/value/guid/value") == Some(guid.to_string().as_str()),
            "unresolved original recovery reference",
        ),
        Some(Value::Error(ValueError::UnexpectedReferenceType { guid, .. })) => require(
            text(cell, "/state") == Some("present")
                && text(cell, "/value/status") == Some("unexpectedTargetType")
                && text(cell, "/value/guid/value") == Some(guid.to_string().as_str()),
            "original recovery reference names an incompatible target",
        ),
        other => state(other, cell),
    }
}

fn check_enum(original: Option<&Value<'_>>, cell: &Json) -> TestResult {
    match original {
        Some(Value::Enum(name)) => require(
            text(cell, "/state") == Some("present") && text(cell, "/value") == Some(*name),
            "exact authored recovery enum",
        ),
        other => state(other, cell),
    }
}

fn check_notices(original: &StructValue<'_>, cell: &Json, catalog: &Json) -> TestResult {
    require(
        text(cell, "/state") == Some("present"),
        "readable original notification block",
    )?;
    let texts = catalog
        .pointer("/texts/gameTexts")
        .and_then(Json::as_array)
        .ok_or("protected text rows")?;
    let notices = member(cell, "value")?;
    for name in NOTIFICATIONS {
        let target = member(notices, name)?;
        match original.field(name) {
            Some(Value::Locale(key)) => {
                let identifier =
                    text(target, "/value").ok_or("original notification text identifier")?;
                let row = texts
                    .iter()
                    .find(|row| text(row, "/id/value") == Some(identifier))
                    .ok_or("authored recovery notification resolves to protected text")?;
                require(
                    text(target, "/state") == Some("present")
                        && text(row, "/key/value") == Some(*key),
                    "exact original notification localisation key",
                )?;
            }
            other => state(other, target)?,
        }
    }
    let buffer = member(notices, BUFFER)?;
    match original.field(BUFFER) {
        Some(Value::Single(number)) => require(
            text(buffer, "/state") == Some("present")
                && text(buffer, "/value/rawBits")
                    == Some(format!("{:08x}", number.to_bits()).as_str()),
            "exact original notification-buffer bits",
        ),
        other => state(other, buffer),
    }
}

pub(super) fn state(original: Option<&Value<'_>>, cell: &Json) -> TestResult {
    let expected = match original {
        None => "absent",
        Some(Value::Null) => "null",
        Some(Value::Error(_)) => "unreadable",
        Some(_) => return Err("unsupported original recovery shape in private oracle".into()),
    };
    require(
        text(cell, "/state") == Some(expected),
        "exact original recovery source state",
    )
}

pub(super) fn require(passed: bool, detail: &'static str) -> TestResult {
    if passed { Ok(()) } else { Err(detail.into()) }
}

pub(super) fn member<'value>(object: &'value Json, name: &str) -> TestResult<&'value Json> {
    object
        .get(name)
        .ok_or_else(|| "required recovery schema member".into())
}

pub(super) fn text<'value>(object: &'value Json, path: &str) -> Option<&'value str> {
    object.pointer(path).and_then(Json::as_str)
}
