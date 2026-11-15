// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{
    BankDialogueEventRow, BankHircContainerNode, BankHircDialogueEvent, BankNodePropertyKind,
    BankNodePropertyValue, BankSwitchMembership,
};
use serde_json::json;

use super::values::{Projection, Rows};
use super::{Input, TestResult};

#[path = "bank_decision_witness_blend.rs"]
mod blend;

pub(super) use self::blend::blend;

pub(super) fn random_sequence(
    node: &mut BankHircContainerNode<'_, Input<'_>>,
    projection: &Projection<'_>,
) -> TestResult<bool> {
    let mut rows = projection.rows("/rows")?;
    let header = projection
        .document
        .get("fields")
        .ok_or("Random/Sequence header")?;
    if let Some(fields) = &node.fields().random_sequence {
        projection.optional(header, "/mode", node.fields().mode.as_ref())?;
        projection.optional(header, "/child_count", node.fields().child_count.as_ref())?;
        for (path, word) in [
            ("/loop_count", fields.loop_count.as_ref()),
            ("/loop_minimum", fields.loop_minimum.as_ref()),
            ("/loop_maximum", fields.loop_maximum.as_ref()),
            ("/avoid_repeat_count", fields.avoid_repeat_count.as_ref()),
        ] {
            projection.optional(header, path, word)?;
        }
        for (path, word) in [
            ("/transition_time/bits", fields.transition_time.as_ref()),
            (
                "/transition_minimum/bits",
                fields.transition_minimum.as_ref(),
            ),
            (
                "/transition_maximum/bits",
                fields.transition_maximum.as_ref(),
            ),
        ] {
            projection.optional(header, path, word)?;
        }
        projection.optional(header, "/transition_mode", fields.transition_mode.as_ref())?;
        projection.optional(header, "/random_mode", fields.random_mode.as_ref())?;
        projection.optional(header, "/flags", fields.flags.as_ref())?;
        if let Some(playlist) = &fields.playlist {
            projection.field(header, "/playlist/declared_count", &playlist.declared_count)?;
            projection.at(header, "/playlist/complete", &json!(playlist.is_complete()))?;
        }
    }
    let mut weighted = false;
    if let Some(playlist) = node.playlist() {
        for original in playlist {
            let original = original?;
            let row = rows.one()?;
            projection.at(row, "/ordinal", &json!(original.ordinal))?;
            projection.field(row, "/play_id", &original.play_id)?;
            projection.optional(row, "/weight_bits", original.weight_bits.as_ref())?;
            if let Some(weight) = original.signed_weight() {
                projection.at(row, "/weight", &json!(weight))?;
                weighted = true;
            }
        }
    }
    rows.finish()?;
    Ok(weighted)
}

pub(super) fn switch(
    node: &mut BankHircContainerNode<'_, Input<'_>>,
    projection: &Projection<'_>,
) -> TestResult<bool> {
    switch_header(node, projection)?;
    let mut rows = projection.rows("/rows")?;
    let mut member_observed = false;
    if let Some(groups) = node.switch_groups() {
        for original in groups {
            match original? {
                BankSwitchMembership::Group(original) => {
                    let row = rows.kind("group")?;
                    projection.at(row, "/ordinal", &json!(original.ordinal))?;
                    projection.field(row, "/value_id", &original.value_id)?;
                    projection.optional(row, "/member_count", original.member_count.as_ref())?;
                }
                BankSwitchMembership::Member(original) => {
                    let row = rows.kind("member")?;
                    projection.at(row, "/group_ordinal", &json!(original.group_ordinal))?;
                    projection.at(row, "/ordinal", &json!(original.ordinal))?;
                    projection.field(row, "/child_id", &original.child_id)?;
                    member_observed = true;
                }
                _ => return Err("known physical Switch membership row".into()),
            }
        }
    }
    let mut configuration_observed = false;
    if let Some(configurations) = node.switch_configurations() {
        for original in configurations {
            let original = original?;
            let row = rows.kind("configuration")?;
            projection.at(row, "/ordinal", &json!(original.ordinal))?;
            projection.field(row, "/child_id", &original.child_id)?;
            projection.optional(row, "/flags", original.flags.as_ref())?;
            projection.optional(row, "/fade_out/bits", original.fade_out_bits.as_ref())?;
            projection.optional(row, "/fade_in/bits", original.fade_in_bits.as_ref())?;
            configuration_observed = true;
        }
    }
    rows.finish()?;
    Ok(member_observed && configuration_observed)
}

pub(super) fn dialogue(
    node: &mut BankHircDialogueEvent<'_, Input<'_>>,
    projection: &Projection<'_>,
) -> TestResult<bool> {
    let fields = node.fields();
    let header = projection
        .document
        .get("fields")
        .ok_or("DialogueEvent header")?;
    projection.optional(header, "/probability", fields.probability.as_ref())?;
    projection.optional(header, "/argument_count", fields.argument_count.as_ref())?;
    projection.optional(header, "/tree_size", fields.tree_size.as_ref())?;
    projection.optional(header, "/mode_code", fields.mode_code.as_ref())?;
    if let Some(tree) = &fields.tree {
        projection.at(
            header,
            "/tree/complete_nodes",
            &json!(tree.complete_nodes()),
        )?;
    }

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
                argument = true;
            }
            BankDialogueEventRow::Node { ordinal, value } => {
                let row = rows.kind("node")?;
                projection.at(row, "/ordinal", &json!(ordinal))?;
                projection.field(row, "/key", &value.key)?;
                projection.optional(row, "/link_word", value.link_word.as_ref())?;
                projection.optional(row, "/weight", value.weight.as_ref())?;
                projection.optional(row, "/probability", value.probability.as_ref())?;
                if value.link_word.is_some() {
                    projection.at(row, "/link_interpretation", &json!("undetermined"))?;
                    physical = true;
                }
            }
            _ => return Err("known physical DialogueEvent row".into()),
        }
    }
    dialogue_properties(node, projection, &mut rows)?;
    rows.finish()?;
    Ok(argument && physical)
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

fn switch_header(
    node: &BankHircContainerNode<'_, Input<'_>>,
    projection: &Projection<'_>,
) -> TestResult {
    let header = projection.document.get("fields").ok_or("Switch header")?;
    if let Some(fields) = &node.fields().switch {
        projection.optional(header, "/group_type", fields.group_type.as_ref())?;
        projection.optional(header, "/group_id", fields.group_id.as_ref())?;
        projection.optional(
            header,
            "/default_value_id",
            fields.default_value_id.as_ref(),
        )?;
        projection.optional(
            header,
            "/continuous_validation",
            fields.continuous_validation.as_ref(),
        )?;
        if let Some(groups) = &fields.groups {
            projection.field(header, "/groups/declared_count", &groups.declared_count)?;
        }
        if let Some(configurations) = &fields.configurations {
            projection.field(
                header,
                "/configurations/declared_count",
                &configurations.declared_count,
            )?;
        }
    }
    Ok(())
}
