// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{
    BankHircDialogueEvent, BankHircObject, BankHircSemantic, BankObservedHeader, HircKind,
};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::bank::Bank;

#[path = "hirc_dialogue_dto.rs"]
mod dto;
#[path = "hirc_dialogue_origins.rs"]
mod origins;
#[path = "hirc_dialogue_rows.rs"]
mod rows;
#[path = "hirc_dialogue_summary.rs"]
mod summary;

pub(super) use self::dto::ObjectSourceSchema;
use self::dto::{
    DialogueSourceDocument, DialogueSourceKind, DialogueSourceLayout, DialogueSourceRow,
    DialogueSummary,
};
use self::origins::DialogueOrigins;
use self::rows::DialogueRows;

/// A physical HIRC declaration read from the owning soundbank during serialization.
#[derive(Debug)]
pub struct ObjectSourceFields<'bank> {
    bank: &'bank Bank,
    index: usize,
}

impl<'bank> ObjectSourceFields<'bank> {
    pub(super) const fn new(bank: &'bank Bank, index: usize) -> Self {
        Self { bank, index }
    }
}

impl Serialize for ObjectSourceFields<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let origins = DialogueOrigins::new(self.bank).map_err(S::Error::custom)?;
        self.bank
            .soundbank()
            .with_hirc_source_fields(self.index, |header, observed| match observed {
                BankHircSemantic::DialogueEvent(dialogue) => {
                    serialize_dialogue(serializer, &origins, &header, *dialogue)
                }
                BankHircSemantic::Raw(object)
                    if object.kind(header.header().version) == HircKind::DialogueEvent =>
                {
                    serialize_unsupported(serializer, &origins, &header, &object)
                }
                _ => Err(S::Error::custom(
                    "the selected original object is not a DialogueEvent",
                )),
            })
            .map_err(S::Error::custom)?
            .ok_or_else(|| S::Error::custom("the selected original object is absent"))?
    }
}

fn serialize_dialogue<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &DialogueOrigins,
    header: &BankObservedHeader,
    dialogue: BankHircDialogueEvent<'_, R>,
) -> Result<S::Ok, S::Error> {
    let original = dialogue.object();
    let error = dialogue.format_error();
    let layout = if error.is_some() {
        DialogueSourceLayout::Malformed
    } else {
        DialogueSourceLayout::Decoded
    };
    let error = error
        .map(|error| origins.coverage(error.to_string(), header, original))
        .transpose()
        .map_err(S::Error::custom)?;
    let layout = origins
        .coverage(layout, header, original)
        .map_err(S::Error::custom)?;
    let fields = summary::collect(&dialogue, origins).map_err(S::Error::custom)?;
    DialogueSourceDocument {
        kind: DialogueSourceKind::DialogueEvent,
        layout,
        version: origins.version(header).map_err(S::Error::custom)?,
        error,
        fields,
        rows: DialogueRows::new(dialogue, origins),
        source_inventory: origins,
    }
    .serialize(serializer)
}

fn serialize_unsupported<S: Serializer>(
    serializer: S,
    origins: &DialogueOrigins,
    header: &BankObservedHeader,
    object: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    DialogueSourceDocument {
        kind: DialogueSourceKind::DialogueEvent,
        layout: origins
            .coverage(DialogueSourceLayout::Unsupported, header, object)
            .map_err(S::Error::custom)?,
        version: origins.version(header).map_err(S::Error::custom)?,
        error: None,
        fields: DialogueSummary::default(),
        rows: [] as [DialogueSourceRow; 0],
        source_inventory: origins,
    }
    .serialize(serializer)
}
