// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_audio::{
    BankDialogueEventFields, BankDialogueEventRow, BankDialogueNode, BankHircDialogueEvent,
    BankHircField, BankNodePropertyKind, BankNodePropertyValue,
};
use serde_json::{Value, json};

use super::values::{Projection, Rows};
use super::{Input, TestResult, require};

pub(super) fn dialogue(
    node: &mut BankHircDialogueEvent<'_, Input<'_>>,
    projection: &Projection<'_>,
) -> TestResult<bool> {
    let extent = dialogue_header(projection, node.fields())?;
    let mut rows = projection.rows("/rows")?;
    let mut argument = false;
    let mut physical = false;
    for original in node.rows() {
        match original? {
            BankDialogueEventRow::Argument {
                ordinal,
                group_id,
                group_type_code,
            } => {
                let row = rows.kind("argument")?;
                projection.at(row, "/ordinal", &json!(ordinal))?;
                projection.field(row, "/group_id", &group_id)?;
                projection.optional(row, "/group_type_code", group_type_code.as_ref())?;
                dialogue_label(
                    projection,
                    row,
                    "/group_type",
                    group_type_code.as_ref(),
                    "switch",
                    "state",
                )?;
                argument = true;
            }
            BankDialogueEventRow::Node { ordinal, value } => {
                let row = rows.kind("node")?;
                let extent = extent
                    .as_ref()
                    .ok_or("original physical DialogueEvent extent")?;
                physical |= extent.check_node(projection, row, ordinal, &value)?;
            }
            _ => return Err("known physical DialogueEvent row".into()),
        }
    }
    dialogue_properties(node, projection, &mut rows)?;
    rows.finish()?;
    Ok(argument && physical)
}

fn dialogue_header(
    projection: &Projection<'_>,
    fields: &BankDialogueEventFields,
) -> TestResult<Option<DialogueWindow>> {
    let header = projection
        .document
        .get("fields")
        .ok_or("DialogueEvent header")?;
    projection.optional(header, "/probability", fields.probability.as_ref())?;
    projection.optional(header, "/argument_count", fields.argument_count.as_ref())?;
    projection.optional(header, "/tree_size", fields.tree_size.as_ref())?;
    projection.optional(header, "/mode_code", fields.mode_code.as_ref())?;
    dialogue_label(
        projection,
        header,
        "/mode",
        fields.mode_code.as_ref(),
        "best-match",
        "weighted",
    )?;
    if let (Some(tree), Some(size)) = (&fields.tree, &fields.tree_size) {
        let extent = DialogueWindow {
            declared: u64::from(size.value) / DIALOGUE_NODE_BYTES,
            admitted: (tree.range.end - tree.range.start) / DIALOGUE_NODE_BYTES,
            size: size.range.clone(),
            prefix: tree.range.clone(),
        };
        projection.at(header, "/tree/complete_nodes", &json!(extent.admitted))?;
        projection.at(header, "/tree/declared_nodes", &json!(extent.declared))?;
        projection.receipt(
            header
                .pointer("/tree/declared_nodes")
                .ok_or("declared table evidence")?,
            &extent.size,
            "conversion",
        )?;
        Ok(Some(extent))
    } else {
        Ok(None)
    }
}

const DIALOGUE_NODE_BYTES: u64 = 12;

struct DialogueWindow {
    declared: u64,
    admitted: u64,
    size: Range<u64>,
    prefix: Range<u64>,
}

impl DialogueWindow {
    fn check_node(
        &self,
        projection: &Projection<'_>,
        row: &Value,
        ordinal: u32,
        node: &BankDialogueNode,
    ) -> TestResult<bool> {
        projection.at(row, "/ordinal", &json!(ordinal))?;
        projection.field(row, "/key", &node.key)?;
        let key_role = match (ordinal, node.key.value) {
            (0, _) => "root",
            (_, 0) => "default-candidate",
            _ => "value-candidate",
        };
        projection.at(row, "/key_role", &json!(key_role))?;
        let role = row.get("key_role").ok_or("physical key role")?;
        projection.receipt(role, &node.key.range, "conversion")?;
        projection.receipt(role, &self.prefix, "selector")?;
        projection.optional(row, "/link_word", node.link_word.as_ref())?;
        projection.optional(row, "/weight", node.weight.as_ref())?;
        projection.optional(row, "/probability", node.probability.as_ref())?;
        if let Some(link) = &node.link_word {
            projection.at(row, "/link_interpretation", &json!("undetermined"))?;
            self.check_link(projection, row, link)?;
            Ok(true)
        } else {
            require(
                row.get("child_candidate").is_none() && row.get("target_candidate").is_none(),
                "a partial link has no candidate",
            )?;
            Ok(false)
        }
    }

    fn check_link(
        &self,
        projection: &Projection<'_>,
        row: &Value,
        link: &BankHircField<u32>,
    ) -> TestResult {
        let [a, b, c, d] = link.value.to_le_bytes();
        let first = u64::from(u16::from_le_bytes([a, b]));
        let count = u64::from(u16::from_le_bytes([c, d]));
        let admitted = self.admitted.saturating_sub(first).min(count);
        let coverage = match (first + count > self.declared, count, admitted == count) {
            (true, _, _) => "outside-declared",
            (_, 0, _) => "empty",
            (_, _, true) => "complete",
            _ => "truncated",
        };
        let child = row
            .get("child_candidate")
            .ok_or("original child-window candidate")?;
        let split = link.range.start + 2;
        for (key, value, range) in [
            ("first", first, link.range.start..split),
            ("count", count, split..link.range.end),
        ] {
            projection.at(child, &format!("/{key}"), &json!(value))?;
            projection.receipt(child.get(key).ok_or("child half-word")?, &range, "payload")?;
        }
        for (path, expected, field) in [
            ("/coverage", json!(coverage), "coverage"),
            ("/admitted_count", json!(admitted), "admitted_count"),
        ] {
            projection.at(child, path, &expected)?;
            let datum = child.get(field).ok_or("child extent evidence")?;
            projection.receipt(datum, &link.range, "conversion")?;
            projection.receipt(datum, &self.size, "selector")?;
            projection.receipt(datum, &self.prefix, "selector")?;
        }
        if link.value == 0 {
            require(
                row.get("target_candidate").is_none(),
                "zero has no target candidate",
            )
        } else {
            projection.at(row, "/target_candidate", &json!(link.value))?;
            projection.receipt(
                row.get("target_candidate")
                    .ok_or("original target candidate")?,
                &link.range,
                "conversion",
            )
        }
    }
}

fn dialogue_label(
    projection: &Projection<'_>,
    row: &Value,
    path: &str,
    code: Option<&BankHircField<u8>>,
    zero: &str,
    one: &str,
) -> TestResult {
    let Some(code) = code else {
        return require(row.pointer(path).is_none(), "an unread code has no label");
    };
    let label = match code.value {
        0 => zero,
        1 => one,
        _ => "unknown",
    };
    projection.at(row, path, &json!(label))?;
    projection.receipt(
        row.pointer(path).ok_or("code label")?,
        &code.range,
        "conversion",
    )
}

fn dialogue_properties(
    node: &mut BankHircDialogueEvent<'_, Input<'_>>,
    projection: &Projection<'_>,
    rows: &mut Rows<'_>,
) -> TestResult {
    for (kind, name) in [
        (BankNodePropertyKind::Properties, "properties"),
        (BankNodePropertyKind::RangedModifiers, "ranged-modifiers"),
    ] {
        let Some(properties) = node.properties(kind) else {
            continue;
        };
        for property in properties {
            let property = property?;
            let row = rows.kind("property")?;
            projection.at(row, "/list", &json!(name))?;
            projection.at(row, "/ordinal", &json!(property.ordinal))?;
            projection.field(row, "/property_id", &property.id)?;
            match property.value {
                BankNodePropertyValue::Scalar(bits) => {
                    projection.optional(row, "/scalar_bits", bits.as_ref())?;
                }
                BankNodePropertyValue::Ranged {
                    minimum, maximum, ..
                } => {
                    projection.optional(row, "/minimum_bits", minimum.as_ref())?;
                    projection.optional(row, "/maximum_bits", maximum.as_ref())?;
                }
                _ => return Err("known DialogueEvent property disposition".into()),
            }
        }
    }
    Ok(())
}
