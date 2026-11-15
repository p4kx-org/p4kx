// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_audio::{
    BankDialogueTree, BankDialogueTreeCoverage, BankHircDialogueEvent, BankNodePropertyKind,
};

use crate::bank::DialogueDecisionMode;
use crate::hirc_view::node_property::PropertyExtent;
use crate::source_evidence_error::SourceEvidenceError;

use super::dto::{DialogueSummary, DialogueTreeStatus, DialogueTreeSummary};
use super::origins::ObjectOrigins;

pub(super) fn collect<R>(
    dialogue: &BankHircDialogueEvent<'_, R>,
    origins: &ObjectOrigins,
) -> Result<DialogueSummary, SourceEvidenceError> {
    let fields = dialogue.fields();
    let checked_arguments = if let (Some(count), Some(range)) =
        (&fields.argument_count, dialogue.argument_ids_range())
    {
        let extent = count.range.start..range.end;
        Some(origins.derived(
            dialogue.checked_arguments(),
            &extent,
            "dialogueCheckedArguments",
        )?)
    } else {
        None
    };
    let checked_argument_types = dialogue
        .argument_types()
        .map(|(extent, count)| origins.derived(count, &extent, "dialogueCheckedArgumentTypes"))
        .transpose()?;
    let tree = if let (Some(tree), Some(size)) = (&fields.tree, &fields.tree_size) {
        Some(tree_summary(tree, &size.range, origins)?)
    } else {
        None
    };
    Ok(DialogueSummary {
        probability: origins.optional(fields.probability.as_ref(), "dialogueProbability")?,
        argument_count: origins
            .optional(fields.argument_count.as_ref(), "dialogueArgumentCount")?,
        checked_arguments,
        checked_argument_types,
        tree_size: origins.optional(fields.tree_size.as_ref(), "dialogueTreeSize")?,
        mode_code: origins.optional(fields.mode_code.as_ref(), "dialogueModeCode")?,
        mode: fields
            .mode_code
            .as_ref()
            .map(|code| {
                origins.derived(
                    DialogueDecisionMode::from(code.value),
                    &code.range,
                    "dialogueModeCode",
                )
            })
            .transpose()?,
        tree,
        properties: fields
            .properties
            .get(BankNodePropertyKind::Properties)
            .map(|extent| PropertyExtent::read(origins, extent))
            .transpose()?,
        ranged_modifiers: fields
            .properties
            .get(BankNodePropertyKind::RangedModifiers)
            .map(|extent| PropertyExtent::read(origins, extent))
            .transpose()?,
    })
}

fn tree_summary(
    tree: &BankDialogueTree,
    size: &Range<u64>,
    origins: &ObjectOrigins,
) -> Result<DialogueTreeSummary, SourceEvidenceError> {
    let extent = size.start..tree.range.end;
    let coverage = match tree.coverage {
        BankDialogueTreeCoverage::Complete => DialogueTreeStatus::Complete,
        BankDialogueTreeCoverage::Truncated => DialogueTreeStatus::Truncated,
        BankDialogueTreeCoverage::Misaligned => DialogueTreeStatus::Misaligned,
        _ => DialogueTreeStatus::Unsupported,
    };
    let tail = tree.partial_node_range();
    Ok(DialogueTreeSummary {
        range: origins.range(&tree.range, "dialogueTreeExtent")?,
        coverage: origins.derived(coverage, &extent, "dialogueTreeCoverage")?,
        declared_nodes: origins.derived(tree.declared_nodes(), size, "dialogueTreeSize")?,
        complete_nodes: origins.derived(tree.complete_nodes(), &extent, "dialogueCompleteNodes")?,
        partial_node_range: tail
            .as_ref()
            .map(|tail| origins.range(tail, "dialoguePartialNodeExtent"))
            .transpose()?,
    })
}
