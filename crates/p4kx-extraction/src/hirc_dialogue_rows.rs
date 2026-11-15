// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_audio::{
    BankDialogueChildCandidate, BankDialogueEventRow, BankDialogueNode, BankHircDialogueEvent,
    BankHircField, BankNodePropertyKind, BankNodePropertyRow,
};
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::bank::{
    DialogueChildCoverage, DialogueGroupType, DialogueKeyRole, DialogueLinkInterpretation,
};
use crate::hirc_view::node_property::PropertyWords;
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_use::{SourceUse, SourceUseRole};

use super::dto::{DialogueChildCandidate, DialoguePropertyKind, DialogueSourceRow};
use super::origins::ObjectOrigins;

pub(super) struct DialogueRows<'view, 'input, R> {
    dialogue: RefCell<BankHircDialogueEvent<'input, R>>,
    origins: &'view ObjectOrigins,
}

impl<'view, 'input, R> DialogueRows<'view, 'input, R> {
    pub(super) const fn new(
        dialogue: BankHircDialogueEvent<'input, R>,
        origins: &'view ObjectOrigins,
    ) -> Self {
        Self {
            dialogue: RefCell::new(dialogue),
            origins,
        }
    }
}

impl<R: Read + Seek> Serialize for DialogueRows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut dialogue = self.dialogue.try_borrow_mut().map_err(S::Error::custom)?;
        let tree = TreeEvidence::of(&dialogue);
        let mut sequence = serializer.serialize_seq(None)?;
        for row in dialogue.rows() {
            let row = match row.map_err(S::Error::custom)? {
                BankDialogueEventRow::Argument {
                    ordinal,
                    group_id,
                    group_type_code,
                } => argument(self.origins, ordinal, &group_id, group_type_code.as_ref())
                    .map_err(S::Error::custom)?,
                BankDialogueEventRow::Node { ordinal, value } => {
                    let extent = tree.as_ref().ok_or_else(|| {
                        S::Error::custom("physical DialogueEvent node lacks its tree extent")
                    })?;
                    node::<S::Error>(self.origins, extent, ordinal, &value)?
                }
                _ => return Err(S::Error::custom("unexpected physical DialogueEvent row")),
            };
            sequence.serialize_element(&row)?;
        }
        self.properties(&mut dialogue, &mut sequence)?;
        sequence.end()
    }
}

impl<R: Read + Seek> DialogueRows<'_, '_, R> {
    fn properties<S: SerializeSeq>(
        &self,
        dialogue: &mut BankHircDialogueEvent<'_, R>,
        sequence: &mut S,
    ) -> Result<(), S::Error> {
        for (kind, source) in [
            (
                DialoguePropertyKind::Properties,
                BankNodePropertyKind::Properties,
            ),
            (
                DialoguePropertyKind::RangedModifiers,
                BankNodePropertyKind::RangedModifiers,
            ),
        ] {
            let Some(rows) = dialogue.properties(source) else {
                continue;
            };
            let count = rows.extent().declared_count.range.clone();
            for row in rows {
                let row = row.map_err(S::Error::custom)?;
                let row = property::<S::Error>(self.origins, &count, kind, &row)?;
                sequence.serialize_element(&row)?;
            }
        }
        Ok(())
    }
}

fn argument(
    origins: &ObjectOrigins,
    ordinal: u32,
    id: &BankHircField<u32>,
    kind: Option<&BankHircField<u8>>,
) -> Result<DialogueSourceRow, SourceEvidenceError> {
    Ok(DialogueSourceRow::Argument {
        ordinal: origins.derived(ordinal, &id.range, "dialogueArgumentPosition")?,
        group_id: origins.field(id, "dialogueGroupId")?,
        group_type_code: origins.optional(kind, "dialogueGroupTypeCode")?,
        group_type: kind
            .map(|code| {
                origins.derived(
                    DialogueGroupType::from(code.value),
                    &code.range,
                    "dialogueGroupTypeCode",
                )
            })
            .transpose()?,
    })
}

struct TreeEvidence {
    size: Range<u64>,
    prefix: Range<u64>,
}

impl TreeEvidence {
    fn of<R>(dialogue: &BankHircDialogueEvent<'_, R>) -> Option<Self> {
        let fields = dialogue.fields();
        Some(Self {
            size: fields.tree_size.as_ref()?.range.clone(),
            prefix: fields.tree.as_ref()?.range.clone(),
        })
    }

    fn prefix(&self, origins: &ObjectOrigins) -> Result<SourceUse, SourceEvidenceError> {
        origins.reference("dialogueTreeExtent", &self.prefix, SourceUseRole::Selector)
    }
}

fn node<E: serde::ser::Error>(
    origins: &ObjectOrigins,
    extent: &TreeEvidence,
    ordinal: u32,
    node: &BankDialogueNode,
) -> Result<DialogueSourceRow, E> {
    let link_interpretation = node
        .link_word
        .as_ref()
        .map(|link| {
            origins.derived(
                DialogueLinkInterpretation::Undetermined,
                &link.range,
                "dialogueNodeLinkWord",
            )
        })
        .transpose()
        .map_err(E::custom)?;
    let key_role = DialogueKeyRole::try_from(node.key_role).map_err(E::custom)?;
    let key_source = origins
        .reference(
            "dialogueNodeKey",
            &node.key.range,
            SourceUseRole::Conversion,
        )
        .map_err(E::custom)?;
    let prefix = extent.prefix(origins).map_err(E::custom)?;
    let child_candidate = node
        .child_candidate
        .as_ref()
        .map(|candidate| child_candidate::<E>(origins, extent, candidate, &prefix))
        .transpose()?;
    let target_candidate = node
        .target_candidate
        .as_ref()
        .map(|target| origins.derived(target.value, &target.range, "dialogueNodeLinkWord"))
        .transpose()
        .map_err(E::custom)?;
    Ok(DialogueSourceRow::Node {
        ordinal: origins
            .derived(ordinal, &node.key.range, "dialogueNodePosition")
            .map_err(E::custom)?,
        range: origins
            .range(&node.range, "dialogueNodeExtent")
            .map_err(E::custom)?,
        key: origins
            .field(&node.key, "dialogueNodeKey")
            .map_err(E::custom)?,
        key_role: origins.value(key_role, &[key_source, prefix]),
        link_word: origins
            .optional(node.link_word.as_ref(), "dialogueNodeLinkWord")
            .map_err(E::custom)?,
        link_interpretation,
        child_candidate,
        target_candidate,
        weight: origins
            .optional(node.weight.as_ref(), "dialogueNodeWeight")
            .map_err(E::custom)?,
        probability: origins
            .optional(node.probability.as_ref(), "dialogueNodeProbability")
            .map_err(E::custom)?,
    })
}

fn child_candidate<E: serde::ser::Error>(
    origins: &ObjectOrigins,
    extent: &TreeEvidence,
    candidate: &BankDialogueChildCandidate,
    prefix: &SourceUse,
) -> Result<DialogueChildCandidate, E> {
    let link = candidate.first.range.start..candidate.count.range.end;
    let link = origins
        .reference("dialogueNodeLinkWord", &link, SourceUseRole::Conversion)
        .map_err(E::custom)?;
    let size = origins
        .reference("dialogueTreeSize", &extent.size, SourceUseRole::Selector)
        .map_err(E::custom)?;
    let uses = [link, size, prefix.clone()];
    let coverage = DialogueChildCoverage::try_from(candidate.coverage).map_err(E::custom)?;
    Ok(DialogueChildCandidate {
        first: origins
            .field(&candidate.first, "dialogueNodeChildFirst")
            .map_err(E::custom)?,
        count: origins
            .field(&candidate.count, "dialogueNodeChildCount")
            .map_err(E::custom)?,
        admitted_count: origins.value(candidate.admitted_count, &uses),
        coverage: origins.value(coverage, &uses),
    })
}

fn property<E: serde::ser::Error>(
    origins: &ObjectOrigins,
    count: &Range<u64>,
    kind: DialoguePropertyKind,
    row: &BankNodePropertyRow,
) -> Result<DialogueSourceRow, E> {
    let words = PropertyWords::read(origins, row)?;
    Ok(DialogueSourceRow::Property {
        list: origins
            .derived(kind, count, "nodePropertyWordLayout")
            .map_err(E::custom)?,
        ordinal: words.ordinal,
        property_id: words.property_id,
        scalar_bits: words.scalar_bits,
        minimum_bits: words.minimum_bits,
        maximum_bits: words.maximum_bits,
    })
}
