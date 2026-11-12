// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{
    BankContainerRead, BankError, BankHircContainerNode, BankHircObject, BankObservedHeader,
    BankRandomSequenceFields, BankRandomSequenceItem,
};
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::source_evidence_error::SourceEvidenceError;

#[path = "hirc_random_sequence_dto.rs"]
mod dto;
#[path = "hirc_random_sequence_fields.rs"]
mod fields;

use super::SourceWire;
use super::dto::DialogueSourceSchema;
use super::origins::ObjectOrigins;

pub(super) use self::dto::RandomSequenceSchema;
use self::dto::{
    RandomSequenceDocument, RandomSequenceFields, RandomSequenceLayout, RandomSequenceRow,
    RandomSequenceSourceKind,
};

pub(super) fn serialize<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    container: BankHircContainerNode<'_, R>,
) -> Result<S::Ok, S::Error> {
    let (layout, error) = coverage(&container);
    let original = container.object();
    let document = RandomSequenceDocument {
        kind: RandomSequenceSourceKind::RandomSequenceContainer,
        layout: origins
            .coverage(layout, header, original)
            .map_err(S::Error::custom)?,
        version: origins.version(header).map_err(S::Error::custom)?,
        error: error
            .map(|error| origins.coverage(error.to_string(), header, original))
            .transpose()
            .map_err(S::Error::custom)?,
        fields: fields::collect(container.fields(), origins).map_err(S::Error::custom)?,
        rows: PlaylistRows {
            container: RefCell::new(container),
            origins,
        },
        source_inventory: origins,
    };
    SourceWire::<DialogueSourceSchema, _>::RandomSequence(document).serialize(serializer)
}

pub(super) fn unsupported<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    original: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    let document = RandomSequenceDocument {
        kind: RandomSequenceSourceKind::RandomSequenceContainer,
        layout: origins
            .coverage(RandomSequenceLayout::Unsupported, header, original)
            .map_err(S::Error::custom)?,
        version: origins.version(header).map_err(S::Error::custom)?,
        error: None,
        fields: RandomSequenceFields::default(),
        rows: [] as [RandomSequenceRow; 0],
        source_inventory: origins,
    };
    SourceWire::<DialogueSourceSchema, _>::RandomSequence(document).serialize(serializer)
}

fn coverage<'view, R>(
    container: &'view BankHircContainerNode<'_, R>,
) -> (RandomSequenceLayout, Option<&'view BankError>) {
    let original = container.fields().random_sequence.as_ref();
    if original
        .and_then(BankRandomSequenceFields::complete_range)
        .is_some()
    {
        return (RandomSequenceLayout::Decoded, None);
    }
    match container.status() {
        BankContainerRead::Malformed(error) if original.is_some() => {
            (RandomSequenceLayout::Malformed, Some(error))
        }
        BankContainerRead::Malformed(error) => (RandomSequenceLayout::Unavailable, Some(error)),
        BankContainerRead::Unsupported(_) => (RandomSequenceLayout::Unsupported, None),
        _ => (RandomSequenceLayout::Unavailable, None),
    }
}

struct PlaylistRows<'view, 'input, R> {
    container: RefCell<BankHircContainerNode<'input, R>>,
    origins: &'view ObjectOrigins,
}

impl<R: Read + Seek> Serialize for PlaylistRows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut container = self.container.try_borrow_mut().map_err(S::Error::custom)?;
        let mut sequence = serializer.serialize_seq(None)?;
        if let Some(rows) = container.playlist() {
            for item in rows {
                let item = item.map_err(S::Error::custom)?;
                let row = row(&item, self.origins).map_err(S::Error::custom)?;
                sequence.serialize_element(&row)?;
            }
        }
        sequence.end()
    }
}

fn row(
    original: &BankRandomSequenceItem,
    origins: &ObjectOrigins,
) -> Result<RandomSequenceRow, SourceEvidenceError> {
    let weight = original
        .weight_bits
        .as_ref()
        .zip(original.signed_weight())
        .map(|(bits, weight)| origins.derived(weight, &bits.range, "randomSequenceWeight"))
        .transpose()?;
    Ok(RandomSequenceRow {
        ordinal: origins.derived(
            original.ordinal,
            &original.play_id.range,
            "randomSequencePlaylistPosition",
        )?,
        range: origins.range(&original.checked_range, "randomSequencePlaylistItemExtent")?,
        play_id: origins.field(&original.play_id, "randomSequencePlayId")?,
        weight_bits: origins.optional(original.weight_bits.as_ref(), "randomSequenceWeightBits")?,
        weight,
    })
}
