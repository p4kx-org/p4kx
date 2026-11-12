// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::{
    BankDialogueTree, BankDialogueTreeCoverage, BankHircDialogueEvent, BankNodePropertyExtents,
    BankNodePropertyKind,
};

use crate::MediaError;
use crate::audio_bank_dialogue_payload::{
    DialogueDecisionMode, DialogueEventObservations, DialogueTreeCoverage,
};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_node_property_payload::NodePropertyListRecord;
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_property_labels;
use crate::audio_bank_property_list;

pub(super) fn observed<R>(
    dialogue: &BankHircDialogueEvent<'_, R>,
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
) -> Result<DialogueEventObservations, MediaError> {
    let fields = dialogue.fields();
    let probability = sites.scalar(
        origins,
        "observed/probability",
        fields.probability.as_ref(),
        "dialogueProbability",
    )?;
    let argument_count = sites.scalar(
        origins,
        "observed/argument_count",
        fields.argument_count.as_ref(),
        "dialogueArgumentCount",
    )?;
    let (checked_arguments, checked_argument_types) = arguments(dialogue, sites, origins)?;
    let tree_size = sites.scalar(
        origins,
        "observed/tree_size",
        fields.tree_size.as_ref(),
        "dialogueTreeSize",
    )?;
    let mode_code = sites.scalar(
        origins,
        "observed/mode_code",
        fields.mode_code.as_ref(),
        "dialogueModeCode",
    )?;
    let mode = if let Some(code) = &fields.mode_code {
        sites.converted(origins, "observed/mode", &code.range, "dialogueModeCode")?;
        Some(DialogueDecisionMode::from(code.value))
    } else {
        None
    };
    let (properties, ranged_modifiers) = properties(sites, origins, &fields.properties)?;
    let mut observed = DialogueEventObservations {
        probability,
        argument_count,
        checked_arguments,
        checked_argument_types,
        tree_size,
        mode_code,
        mode,
        tree_range: None,
        tree_coverage: None,
        complete_nodes: None,
        partial_node_bytes: None,
        partial_node_range: None,
        properties,
        ranged_modifiers,
    };
    if let (Some(size), Some(tree)) = (&fields.tree_size, &fields.tree) {
        tree_extent(&mut observed, sites, origins, size.range.start, tree)?;
    }
    Ok(observed)
}

fn properties(
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
    fields: &BankNodePropertyExtents,
) -> Result<
    (
        Option<NodePropertyListRecord>,
        Option<NodePropertyListRecord>,
    ),
    MediaError,
> {
    let properties = audio_bank_property_list::record(
        sites,
        origins,
        "observed/properties",
        fields.get(BankNodePropertyKind::Properties),
        &audio_bank_property_labels::NODE,
    )?;
    let ranged_modifiers = audio_bank_property_list::record(
        sites,
        origins,
        "observed/ranged_modifiers",
        fields.get(BankNodePropertyKind::RangedModifiers),
        &audio_bank_property_labels::NODE,
    )?;
    Ok((properties, ranged_modifiers))
}

fn arguments<R>(
    dialogue: &BankHircDialogueEvent<'_, R>,
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
) -> Result<(Option<u32>, Option<u32>), MediaError> {
    let ids = match (
        &dialogue.fields().argument_count,
        dialogue.argument_ids_range(),
    ) {
        (Some(count), Some(ids)) => {
            let checked = count.range.start..ids.end;
            sites.converted(
                origins,
                "observed/checked_arguments",
                &checked,
                "dialogueCheckedArguments",
            )?;
            Some(dialogue.checked_arguments())
        }
        _ => None,
    };
    let types = if let Some((extent, count)) = dialogue.argument_types() {
        sites.converted(
            origins,
            "observed/checked_argument_types",
            &extent,
            "dialogueCheckedArgumentTypes",
        )?;
        Some(count)
    } else {
        None
    };
    Ok((ids, types))
}

fn tree_extent(
    observed: &mut DialogueEventObservations,
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
    size_start: u64,
    tree: &BankDialogueTree,
) -> Result<(), MediaError> {
    sites.range(origins, "observed/tree_range", &tree.range)?;
    let inspected = size_start..tree.range.end;
    sites.converted(
        origins,
        "observed/tree_coverage",
        &inspected,
        "dialogueTreeCoverage",
    )?;
    sites.converted(
        origins,
        "observed/complete_nodes",
        &inspected,
        "dialogueCompleteNodes",
    )?;
    sites.converted(
        origins,
        "observed/partial_node_bytes",
        &inspected,
        "dialoguePartialNodeBytes",
    )?;
    let coverage = match tree.coverage {
        BankDialogueTreeCoverage::Complete => DialogueTreeCoverage::Complete,
        BankDialogueTreeCoverage::Truncated => DialogueTreeCoverage::Truncated,
        BankDialogueTreeCoverage::Misaligned => DialogueTreeCoverage::Misaligned,
        _ => {
            return Err(MediaError::Contract(
                "unexpected DialogueEvent tree coverage".into(),
            ));
        }
    };
    let tail = tree.partial_node_range();
    if let Some(tail) = &tail {
        sites.range(origins, "observed/partial_node_range", tail)?;
    }
    observed.tree_range = Some((&tree.range).into());
    observed.tree_coverage = Some(coverage);
    observed.complete_nodes = Some(tree.complete_nodes());
    observed.partial_node_bytes = Some(tree.partial_node_bytes());
    observed.partial_node_range = tail.as_ref().map(Into::into);
    Ok(())
}
