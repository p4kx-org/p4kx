// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{
    BankHircDialogueEvent, BankHircObject, BankHircSemantic, BankObservedHeader, HircKind,
};
use schemars::JsonSchema;
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::bank::Bank;

#[path = "hirc_dialogue_dto.rs"]
mod dto;
#[path = "hirc_source_origins.rs"]
mod origins;
#[path = "hirc_random_sequence_source.rs"]
mod random_sequence;
#[path = "hirc_dialogue_rows.rs"]
mod rows;
#[path = "hirc_dialogue_summary.rs"]
mod summary;

use self::dto::DialogueSourceSchema;
use self::dto::{
    DialogueSourceDocument, DialogueSourceKind, DialogueSourceLayout, DialogueSourceRow,
    DialogueSummary,
};
use self::origins::ObjectOrigins;
use self::random_sequence::RandomSequenceSchema;
use self::rows::DialogueRows;

pub(super) type ObjectSourceSchema = SourceWire<DialogueSourceSchema, RandomSequenceSchema>;

#[derive(Serialize, JsonSchema)]
#[serde(untagged)]
#[schemars(rename = "HircObjectSourceFields")]
pub(super) enum SourceWire<Dialogue, RandomSequence> {
    Dialogue(Dialogue),
    RandomSequence(RandomSequence),
}

#[derive(Debug, Clone, Copy)]
enum SourceFamily {
    Dialogue,
    RandomSequence,
}

impl SourceFamily {
    const fn kind(self) -> HircKind {
        match self {
            Self::Dialogue => HircKind::DialogueEvent,
            Self::RandomSequence => HircKind::RandomSequenceContainer,
        }
    }

    const fn namespace(self) -> &'static str {
        match self {
            Self::Dialogue => "bnkDialogueEvent",
            Self::RandomSequence => "bnkHircRandomSequence",
        }
    }

    const fn extent_label(self) -> &'static str {
        match self {
            Self::Dialogue => "dialogueExtent",
            Self::RandomSequence => "randomSequenceExtent",
        }
    }
}

/// A physical HIRC declaration read from the owning soundbank during serialization.
#[derive(Debug)]
pub struct ObjectSourceFields<'bank> {
    bank: &'bank Bank,
    index: usize,
    family: SourceFamily,
}

impl<'bank> ObjectSourceFields<'bank> {
    pub(super) const fn new(bank: &'bank Bank, index: usize, kind: HircKind) -> Option<Self> {
        let family = match kind {
            HircKind::DialogueEvent => SourceFamily::Dialogue,
            HircKind::RandomSequenceContainer => SourceFamily::RandomSequence,
            _ => return None,
        };
        Some(Self {
            bank,
            index,
            family,
        })
    }
}

impl Serialize for ObjectSourceFields<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let origins = ObjectOrigins::new(self.bank, self.family).map_err(S::Error::custom)?;
        self.bank
            .soundbank()
            .with_hirc_source_fields(self.index, |header, observed| {
                project(serializer, &origins, &header, self.family, observed)
            })
            .map_err(S::Error::custom)?
            .ok_or_else(|| S::Error::custom("the selected original object is absent"))?
    }
}

fn project<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    family: SourceFamily,
    observed: BankHircSemantic<'_, R>,
) -> Result<S::Ok, S::Error> {
    match (observed, family) {
        (BankHircSemantic::DialogueEvent(dialogue), SourceFamily::Dialogue) => {
            serialize_dialogue(serializer, origins, header, *dialogue)
        }
        (BankHircSemantic::ContainerWithNode(container), SourceFamily::RandomSequence) => {
            random_sequence::serialize(serializer, origins, header, *container)
        }
        (BankHircSemantic::Raw(object), family)
            if object.kind(header.header().version) == family.kind() =>
        {
            match family {
                SourceFamily::Dialogue => {
                    serialize_unsupported(serializer, origins, header, &object)
                }
                SourceFamily::RandomSequence => {
                    random_sequence::unsupported(serializer, origins, header, &object)
                }
            }
        }
        _ => Err(S::Error::custom(
            "the selected original object has an unexpected source layout",
        )),
    }
}

fn serialize_dialogue<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
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
    let document = DialogueSourceDocument {
        kind: DialogueSourceKind::DialogueEvent,
        layout,
        version: origins.version(header).map_err(S::Error::custom)?,
        error,
        fields,
        rows: DialogueRows::new(dialogue, origins),
        source_inventory: origins,
    };
    SourceWire::<_, RandomSequenceSchema>::Dialogue(document).serialize(serializer)
}

fn serialize_unsupported<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    object: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    let document = DialogueSourceDocument {
        kind: DialogueSourceKind::DialogueEvent,
        layout: origins
            .coverage(DialogueSourceLayout::Unsupported, header, object)
            .map_err(S::Error::custom)?,
        version: origins.version(header).map_err(S::Error::custom)?,
        error: None,
        fields: DialogueSummary::default(),
        rows: [] as [DialogueSourceRow; 0],
        source_inventory: origins,
    };
    SourceWire::<_, RandomSequenceSchema>::Dialogue(document).serialize(serializer)
}
