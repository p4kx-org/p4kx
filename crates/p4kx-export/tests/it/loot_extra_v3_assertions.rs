// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceInventory;
use serde_json::{Value, json};

use crate::loot_extra_v3_fixture::{self as fixture, Checked};
use crate::loot_extra_v3_graph::{self as graph, Graph};
use crate::loot_extra_v3_source::{Original, Step};

pub(crate) fn check_root_scalars(
    graph: &Graph<'_>,
    fields: &[Value],
    original: &Original,
    inventory: &SourceInventory,
) -> Checked {
    let number = graph.field(fields, 1, "rootExtra", "uint64", false)?;
    graph::scalar(
        number,
        "number",
        &json!({"type":"uint64","value":u64::MAX.to_string()}),
    )?;
    graph::witness(
        number.get("value").ok_or("wide integer sourced cell")?,
        inventory,
        original.ordinal(&[Step::Field(1)])?,
        "$value",
    )?;
    graph::scalar(
        graph.field(fields, 2, "rootFlag", "bool", false)?,
        "boolean",
        &json!(false),
    )?;
    check_text(
        graph.field(fields, 3, "rootEmpty", "string", false)?,
        "string",
        "",
    )?;
    check_text(
        graph.field(fields, 4, "rootLocale", "locale", false)?,
        "locale",
        fixture::LOCALE,
    )?;
    Ok(())
}

pub(crate) fn check_extra_fields(graph: &Graph<'_>, filter: &Value) -> Checked {
    assert_eq!(
        graph::present(
            filter
                .get("sourceType")
                .ok_or("nested original source type")?
        )?,
        &json!("PoolFilter_Custom"),
        "concrete source type is retained"
    );
    let fields = graph::array(filter, "fields")?;
    assert_eq!(
        fields.len(),
        3,
        "all nested fields are retained, not just recognized filter members"
    );
    let array = graph.field(fields, 1, "extraArray", "int32", true)?;
    let elements = graph::array(graph::payload(array, "array")?, "elements")?;
    assert_eq!(elements.len(), 3, "numeric source array cardinality/order");
    for (reference, number) in elements.iter().zip([11, -22, 0]) {
        graph::scalar(
            graph.follow(reference)?,
            "number",
            &json!({"type":"int32","value":number}),
        )?;
    }
    let extra = graph::payload(
        graph.field(fields, 2, "extraObject", "class", false)?,
        "struct",
    )?;
    assert_eq!(
        graph::present(
            extra
                .get("sourceType")
                .ok_or("inline original source type")?
        )?,
        &json!("ExtraPayload"),
        "inline class source identity"
    );
    let fields = graph::array(extra, "fields")?;
    assert_eq!(fields.len(), 2, "all inline class fields survive");
    check_text(
        graph.field(fields, 0, "preserveName", "string", false)?,
        "string",
        fixture::LITERAL,
    )?;
    graph::scalar(
        graph.field(fields, 1, "preserveFalse", "bool", false)?,
        "boolean",
        &json!(false),
    )?;
    Ok(())
}

fn check_text(node: &Value, kind: &str, text: &str) -> Checked {
    let text_value = graph::payload(node, kind)?
        .get("value")
        .ok_or("retained text representation")?;
    assert_eq!(
        text_value.get("sourceText"),
        Some(&json!(text)),
        "literal/key is exact, without trimming, decoding display escapes or evaluation"
    );
    assert_eq!(
        text_value.get("type"),
        Some(&json!("gameText")),
        "unknown string/Locale owner uses the approved conservative domain"
    );
    Ok(())
}

pub(crate) fn check_slots<'catalog>(
    graph: &Graph<'catalog>,
    filter: &Value,
    holes: bool,
    target_index: u16,
) -> Checked<&'catalog [Value]> {
    let fields = graph::array(graph::payload(filter, "struct")?, "fields")?;
    let field = fields.first().ok_or("ordered filters declaration")?;
    let target = graph::present(
        field
            .pointer("/declaration/target")
            .ok_or("sourced actual target")?,
    )?;
    assert_eq!(
        target.as_u64(),
        Some(u64::from(target_index)),
        "actual original target struct index, independent of child value shapes"
    );
    let array = graph.field(fields, 0, "filters", "strongPointer", true)?;
    let slots = graph::array(graph::payload(array, "array")?, "elements")?;
    assert_eq!(
        slots.len(),
        if holes { 4 } else { 2 },
        "no struct-only compaction or null/error filtering"
    );
    check_instance(
        graph,
        graph.follow(slots.first().ok_or("first original element")?)?,
        "first",
        -7,
    )?;
    check_instance(
        graph,
        graph.follow(slots.last().ok_or("last original element")?)?,
        "last",
        -8,
    )?;
    Ok(slots)
}

fn check_instance(graph: &Graph<'_>, node: &Value, name: &str, number: i32) -> Checked {
    let payload = graph::payload(node, "struct")?;
    let fields = graph::array(payload, "fields")?;
    assert_eq!(fields.len(), 3, "all instance declarations remain ordered");
    let label = graph::payload(graph.field(fields, 0, "name", "string", false)?, "string")?;
    assert_eq!(
        label.pointer("/value/sourceText"),
        Some(&json!(name)),
        "first/final readable order is unchanged"
    );
    let mode = graph::payload(graph.field(fields, 1, "mode", "string", false)?, "string")?;
    assert_eq!(
        mode.pointer("/value/sourceText"),
        Some(&json!("Cumulative")),
        "designer mode is retained"
    );
    let leaf = graph::payload(
        graph.field(fields, 2, "filter", "strongPointer", false)?,
        "struct",
    )?;
    let fields = graph::array(leaf, "fields")?;
    assert_eq!(fields.len(), 1, "leaf original extra field");
    graph::scalar(
        graph.field(fields, 0, "leafExtra", "int32", false)?,
        "number",
        &json!({"type":"int32","value":number}),
    )?;
    Ok(())
}

pub(crate) fn check_holes(
    graph: &Graph<'_>,
    slots: &[Value],
    original: &Original,
    inventory: &SourceInventory,
) -> Checked {
    let null = graph.follow(slots.get(1).ok_or("original null array position")?)?;
    let null_cell = null.get("value").ok_or("immediate source null")?;
    graph::cell(null_cell, "null");
    assert_eq!(
        null_cell.get("value"),
        Some(&Value::Null),
        "source null remains explicit null"
    );
    assert_eq!(
        null.get("originalError"),
        Some(&Value::Null),
        "null is not a decoder error"
    );
    graph::witness(
        null_cell,
        inventory,
        original.ordinal(&[Step::Field(0), Step::Field(0), Step::Element(1)])?,
        "$value",
    )?;
    let error = graph.follow(slots.get(2).ok_or("original malformed array position")?)?;
    let error_cell = error.get("value").ok_or("immediate unreadable source")?;
    graph::cell(error_cell, "unreadable");
    assert_eq!(
        error_cell.get("value"),
        Some(&Value::Null),
        "semantic unreadability does not invent a successful value"
    );
    let facts = error
        .get("originalError")
        .ok_or("required original error facts")?;
    assert_eq!(
        facts.get("type"),
        Some(&json!("invalidPointer")),
        "exact decoder variant survives per occurrence"
    );
    let ordinal = original.ordinal(&[Step::Field(0), Step::Field(0), Step::Element(2)])?;
    for (member, value, token) in [
        ("structIndex", 77, "$error.structIndex"),
        ("instanceIndex", 99, "$error.instanceIndex"),
    ] {
        let fact = facts
            .get(member)
            .ok_or("each actual malformed pointer word")?;
        assert_eq!(
            graph::present(fact)?,
            &json!(value),
            "all original error parameters survive without a Health-sample join"
        );
        graph::witness(fact, inventory, ordinal, token)?;
    }
    Ok(())
}
