// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support::{self as support, TestResult};
use crate::gameplay_v3_values as values;
use p4kx_extraction::Value as Raw;
use p4kx_testkit::TestValue;
use serde_json::json;

#[test]
fn gameplay_v3_keeps_all_name_overrides_and_three_condition_forms_with_open_ordered_enum_labels()
-> TestResult {
    let conditions = [
        values::owned(schema::CONDITION),
        values::owned(schema::CONDITION_NON_REF),
        values::item_type(
            vec![
                TestValue::text("SyntheticB"),
                TestValue::text(""),
                TestValue::text("SyntheticB"),
            ],
            vec![
                TestValue::text("SyntheticSub"),
                TestValue::text("SyntheticSub"),
            ],
        ),
    ];
    let database = schema::schema().record(
        values::record(values::FIRST, "Primary", values::owned(schema::BASE)).value(
            "nameOverrides",
            TestValue::Array(
                conditions
                    .into_iter()
                    .map(|condition| values::name_override("@resource_name", condition))
                    .chain(std::iter::once(values::name_override(
                        "@resource_name",
                        TestValue::Null,
                    )))
                    .collect(),
            ),
        ),
    );
    source::declarations(&database)?;
    original_overrides(&database)?;
    let (_, catalog) = support::normal(&database)?;
    assert_overrides(&catalog)?;
    Ok(())
}

fn original_overrides(database: &p4kx_testkit::DataForgeBuilder) -> TestResult {
    source::with_tree(database, 0, |tree| {
        let slots = source::array(tree.root().field("nameOverrides"))?;
        assert_eq!(
            slots.len(),
            4,
            "all actual inline class occurrences are materialized"
        );
        for (slot, expected) in slots.iter().zip([
            schema::CONDITION,
            schema::CONDITION_NON_REF,
            schema::ITEM_TYPE,
        ]) {
            let entry = source::structure(Some(slot))?;
            assert_eq!(
                entry.type_name(),
                schema::OVERRIDE,
                "the Class array has its actual original owner"
            );
            assert_eq!(
                entry.field("propertyName"),
                Some(&Raw::Locale("@resource_name")),
                "original override Locale"
            );
            let condition = source::structure(entry.field("condition"))?;
            assert_eq!(
                condition.type_name(),
                expected,
                "the actual condition pointer type"
            );
        }
        let entry = source::structure(slots.get(2))?;
        let condition = source::structure(entry.field("condition"))?;
        assert_eq!(
            source::array(condition.field("matchItemTypes"))?,
            &[
                Raw::Enum("SyntheticB"),
                Raw::Enum(""),
                Raw::Enum("SyntheticB")
            ],
            "the original open labels include an empty label and repetition"
        );
        let null_entry = source::structure(slots.get(3))?;
        assert!(
            matches!(null_entry.field("condition"), Some(Raw::Null)),
            "the original condition pointer is genuinely null"
        );
        Ok(())
    })?;
    Ok(())
}

fn assert_overrides(catalog: &serde_json::Value) -> TestResult {
    let overrides = support::at(support::row(catalog, 0)?, "/nameOverrides/value")?;
    assert_eq!(
        overrides.as_array().map(Vec::len),
        Some(4),
        "no chosen/merged override"
    );
    for (index, (kind, tag)) in [
        (schema::CONDITION, "emptyBase"),
        (schema::CONDITION_NON_REF, "emptyBase"),
        (schema::ITEM_TYPE, "itemType"),
    ]
    .into_iter()
    .enumerate()
    {
        let entry = support::at(overrides, &format!("/{index}/value"))?;
        assert_eq!(
            support::at(entry, "/sourceType/value")?,
            &json!(schema::OVERRIDE),
            "actual override source class"
        );
        let payload = support::at(entry, "/payload/value")?
            .as_object()
            .ok_or("actual override payload")?;
        assert_eq!(
            payload.len(),
            2,
            "the override owns only its two actual fields"
        );
        assert!(
            !payload.contains_key("type"),
            "a fixed original override class gains no invented operation tag"
        );
        let condition = support::at(entry, "/payload/value/condition/value")?;
        assert_eq!(
            support::at(condition, "/sourceType/value")?,
            &json!(kind),
            "actual condition class is distinct"
        );
        assert_eq!(
            support::at(condition, "/payload/value/type")?,
            &json!(tag),
            "closed condition shape"
        );
    }
    support::state(
        support::at(overrides, "/3/value/payload/value/condition")?,
        "null",
    )?;
    let items = support::at(
        overrides,
        "/2/value/payload/value/condition/value/payload/value/matchItemTypes/value",
    )?;
    for (index, label) in ["SyntheticB", "", "SyntheticB"].into_iter().enumerate() {
        assert_eq!(
            support::at(items, &format!("/{index}/value"))?,
            &json!(label),
            "exact original Enum slot"
        );
    }
    Ok(())
}
