// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{
    BankDialogueChildCandidate, BankDialogueNode, DialogueChildCoverage, DialogueKeyRole,
};

use crate::MediaError;
use crate::audio_bank_dialogue_payload::DialogueNodeChildren;
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_payload::BankField;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;

use super::TreeEvidence;

pub(super) struct Candidates {
    pub(super) key_role: DialogueKeyRole,
    pub(super) child: Option<DialogueNodeChildren>,
    pub(super) target: Option<BankField<u32>>,
}

pub(super) fn observe(
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
    node: &BankDialogueNode,
    tree: &TreeEvidence,
) -> Result<Candidates, MediaError> {
    let key_role = DialogueKeyRole::try_from(node.key_role).map_err(contract)?;
    let position = capture(
        sites,
        origins,
        [
            (&node.key.range, "dialogueNodeKey"),
            (&tree.admitted, "dialogueTreeCoverage"),
        ],
    )?;
    origins.field("/data/payload/key_role", position)?;
    let child = node
        .child_candidate
        .as_ref()
        .map(|child| project_children(sites, origins, child, tree))
        .transpose()?;
    let target = sites.scalar(
        origins,
        "target_candidate",
        node.target_candidate.as_ref(),
        "dialogueNodeLinkWord",
    )?;
    Ok(Candidates {
        key_role,
        child,
        target,
    })
}

fn project_children(
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
    child: &BankDialogueChildCandidate,
    tree: &TreeEvidence,
) -> Result<DialogueNodeChildren, MediaError> {
    sites.field(
        origins,
        "child_candidate/first",
        &child.first.range,
        "dialogueNodeChildFirst",
    )?;
    sites.field(
        origins,
        "child_candidate/count",
        &child.count.range,
        "dialogueNodeChildCount",
    )?;
    let bounds = capture(
        sites,
        origins,
        [
            (&child.first.range, "dialogueNodeChildFirst"),
            (&child.count.range, "dialogueNodeChildCount"),
            (&tree.size, "dialogueTreeSize"),
            (&tree.admitted, "dialogueTreeCoverage"),
        ],
    )?;
    origins.field(
        "/data/payload/child_candidate/admitted_count",
        bounds.clone(),
    )?;
    origins.field("/data/payload/child_candidate/coverage", bounds)?;
    Ok(DialogueNodeChildren {
        first: (&child.first).into(),
        count: (&child.count).into(),
        admitted_count: child.admitted_count,
        coverage: DialogueChildCoverage::try_from(child.coverage).map_err(contract)?,
    })
}

fn capture<const N: usize>(
    sites: &NodeSites,
    origins: &BankOrigins<'_, '_, '_>,
    fields: [(&Range<u64>, &'static str); N],
) -> Result<OriginContributions, MediaError> {
    let positions = fields.map(|(range, label)| ([range.start, range.end], label));
    let field_sites = positions.iter().map(|(indices, label)| {
        (
            OriginSite::new("bnkDialogueEvent", indices, label),
            SourceUseRole::Conversion,
        )
    });
    origins.capture(sites.property_context().into_iter().chain(field_sites))
}

fn contract(context: &'static str) -> MediaError {
    MediaError::Contract(context.into())
}
