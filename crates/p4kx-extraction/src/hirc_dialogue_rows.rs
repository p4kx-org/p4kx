// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_audio::{
    BankDialogueEventRow, BankDialogueNode, BankHircDialogueEvent, BankHircField,
    BankNodePropertyKind, BankNodePropertyRow,
};
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::hirc_view::node_property::PropertyWords;
use crate::source_evidence_error::SourceEvidenceError;

use super::dto::{DialogueLinkInterpretation, DialoguePropertyKind, DialogueSourceRow};
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
        let mut sequence = serializer.serialize_seq(None)?;
        for row in dialogue.rows() {
            let row = match row.map_err(S::Error::custom)? {
                BankDialogueEventRow::Argument {
                    ordinal,
                    group_id,
                    group_type_code,
                } => argument(self.origins, ordinal, &group_id, group_type_code.as_ref()),
                BankDialogueEventRow::Node { ordinal, value } => {
                    node(self.origins, ordinal, &value)
                }
                _ => return Err(S::Error::custom("unexpected physical DialogueEvent row")),
            }
            .map_err(S::Error::custom)?;
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
    })
}

fn node(
    origins: &ObjectOrigins,
    ordinal: u32,
    node: &BankDialogueNode,
) -> Result<DialogueSourceRow, SourceEvidenceError> {
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
        .transpose()?;
    Ok(DialogueSourceRow::Node {
        ordinal: origins.derived(ordinal, &node.key.range, "dialogueNodePosition")?,
        range: origins.range(&node.range, "dialogueNodeExtent")?,
        key: origins.field(&node.key, "dialogueNodeKey")?,
        link_word: origins.optional(node.link_word.as_ref(), "dialogueNodeLinkWord")?,
        link_interpretation,
        weight: origins.optional(node.weight.as_ref(), "dialogueNodeWeight")?,
        probability: origins.optional(node.probability.as_ref(), "dialogueNodeProbability")?,
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
