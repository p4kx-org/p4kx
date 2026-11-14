// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::BankDialogueNode;

use crate::audio_bank_dialogue_payload::{
    DialogueEventPayload, DialogueLinkInterpretation, DialogueNodeLayout, DialogueNodeRecord, NODE,
};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(super) fn project(
    bank: &BankProjection<'_>,
    sites: &NodeSites,
    ordinal: u32,
    value: &BankDialogueNode,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut origins = sites.record(bank.input, writer, Some(&value.key.range))?;
    sites.index(&mut origins, 2, &value.key.range, "dialogueNodePosition")?;
    let data = observe(sites, &mut origins, value)?;
    let (status, issue) = if matches!(data.layout, DialogueNodeLayout::Decoded) {
        (Status::Complete, None)
    } else {
        sites.issues(&mut origins, &value.range)?;
        let issue = MediaIssue::new(
            "audio.bank.incomplete-dialogue-node",
            "HIRC",
            Severity::Warning,
            "the declared dialogue tree ends inside a physical node",
        );
        (Status::Partial, Some(issue))
    };
    let [section, object] = sites.indices();
    origins.finish().write(
        writer,
        &Subresource::new(NODE, vec![section, object, u64::from(ordinal)]),
        status,
        &DialogueEventPayload::Node(&data),
        issue.as_slice(),
    )
}

fn observe(
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
    value: &BankDialogueNode,
) -> Result<DialogueNodeRecord, MediaError> {
    sites.range(origins, "range", &value.range)?;
    sites.converted(origins, "layout", &value.range, "dialogueNodeCoverage")?;
    sites.field(origins, "key", &value.key.range, "dialogueNodeKey")?;
    let link_word = sites.scalar(
        origins,
        "link_word",
        value.link_word.as_ref(),
        "dialogueNodeLinkWord",
    )?;
    let link_interpretation = if let Some(link) = &value.link_word {
        sites.converted(
            origins,
            "link_interpretation",
            &link.range,
            "dialogueNodeLinkWord",
        )?;
        Some(DialogueLinkInterpretation::Undetermined)
    } else {
        None
    };
    let weight = sites.scalar(
        origins,
        "weight",
        value.weight.as_ref(),
        "dialogueNodeWeight",
    )?;
    let probability = sites.scalar(
        origins,
        "probability",
        value.probability.as_ref(),
        "dialogueNodeProbability",
    )?;
    let layout = if probability.is_some() {
        DialogueNodeLayout::Decoded
    } else {
        DialogueNodeLayout::Malformed
    };
    Ok(DialogueNodeRecord {
        layout,
        range: (&value.range).into(),
        key: (&value.key).into(),
        link_word,
        link_interpretation,
        weight,
        probability,
    })
}
