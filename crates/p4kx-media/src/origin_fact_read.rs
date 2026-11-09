// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    FieldOrigin, FieldOriginSeed, OriginReaderLimits, SourceDeclarationEvidence, SourceUseRef,
    SourceValue,
};
use serde::Deserialize;
use serde::de::{DeserializeSeed, Deserializer};

use crate::MediaError;
use crate::origin_read_policy::InventoryBudget;
use crate::origin_source_shape::{DecimalCoordinate, DeclarationKind};
use crate::origin_value_read;

/// One original inventory fact's descriptive immediate origin.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub(crate) struct Fact<T> {
    state: FactState,
    #[serde(default, deserialize_with = "read_nullable")]
    value: Nullable<T>,
    #[serde(default, deserialize_with = "read_nullable")]
    contains_encrypted_source: Nullable<bool>,
    source_uses_refs: Vec<String>,
    source_set_complete: bool,
}

#[derive(PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum FactState {
    Present,
    Absent,
    Null,
    Unreadable,
    Unsupported,
    Invalid,
}

#[derive(Default)]
enum Nullable<T> {
    #[default]
    Missing,
    Present(Option<T>),
}

impl<T> Nullable<T> {
    fn required(self, name: &str) -> Result<Option<T>, MediaError> {
        match self {
            Self::Present(value) => Ok(value),
            Self::Missing => Err(MediaError::Destination(format!(
                "inventory fact lacks required {name}"
            ))),
        }
    }
}

fn read_nullable<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Nullable<T>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Nullable::Present)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Declaration {
    kind: DeclarationKind,
    #[serde(rename = "valueOffset")]
    _value_offset: DecimalCoordinate,
    declared_block_length: Fact<u16>,
    raw_word: Fact<u16>,
    raw_bytes: Fact<Vec<u8>>,
}

impl<T> Fact<T> {
    pub(crate) fn capture_origin(
        self,
        limits: OriginReaderLimits,
        budget: &mut InventoryBudget,
        origins: &mut Vec<FieldOrigin>,
    ) -> Result<(), MediaError> {
        self.capture_value(limits, budget, origins).map(|_| ())
    }

    pub(crate) fn capture_value(
        self,
        limits: OriginReaderLimits,
        budget: &mut InventoryBudget,
        origins: &mut Vec<FieldOrigin>,
    ) -> Result<SourceValue<T>, MediaError> {
        let value = match (self.state, self.value.required("value")?) {
            (FactState::Present, Some(value)) => SourceValue::Present(value),
            (FactState::Absent, None) => SourceValue::Absent,
            (FactState::Null, None) => SourceValue::Null,
            (FactState::Unreadable, None) => SourceValue::Unreadable,
            (FactState::Unsupported, None) => SourceValue::Unsupported,
            (FactState::Invalid, None) => SourceValue::Invalid,
            _ => {
                return Err(MediaError::Destination(
                    "inventory fact state disagrees with its typed value".into(),
                ));
            }
        };
        let marker = self
            .contains_encrypted_source
            .required("containsEncryptedSource")?;
        let origin = FieldOriginSeed::new(limits)
            .deserialize(origin_value_read::deserializer(
                marker,
                self.source_uses_refs,
                self.source_set_complete,
            ))
            .map_err(|cause| MediaError::Destination(cause.to_string()))?;
        budget.charge(size_of::<FieldOrigin>())?;
        for reference in origin.source_uses_refs() {
            budget.charge(reference.as_str().len())?;
            budget.charge(size_of::<SourceUseRef>())?;
        }
        origins.push(origin);
        Ok(value)
    }
}

impl Declaration {
    pub(crate) fn capture(
        self,
        limits: OriginReaderLimits,
        budget: &mut InventoryBudget,
        origins: &mut Vec<FieldOrigin>,
    ) -> Result<SourceDeclarationEvidence, MediaError> {
        let block_length = self
            .declared_block_length
            .capture_value(limits, budget, origins)?;
        let raw_word = self.raw_word.capture_value(limits, budget, origins)?;
        let raw_bytes = self.raw_bytes.capture_value(limits, budget, origins)?;
        let word = combine_word(&raw_word, raw_bytes)?;
        Ok(SourceDeclarationEvidence::new(
            self.kind.shared(),
            block_length,
            word,
        ))
    }
}

fn invalid_bytes() -> MediaError {
    MediaError::Destination("raw declaration word disagrees with available raw bytes".into())
}

#[derive(Clone, Copy)]
enum WordFact {
    Present(u16),
    Missing,
    Unavailable,
}

const fn word_fact(value: &SourceValue<u16>) -> WordFact {
    match value {
        SourceValue::Present(value) => WordFact::Present(*value),
        SourceValue::Absent
        | SourceValue::Null
        | SourceValue::Invalid
        | SourceValue::Unsupported => WordFact::Missing,
        SourceValue::Unreadable | _ => WordFact::Unavailable,
    }
}

fn combine_word(
    word: &SourceValue<u16>,
    bytes: SourceValue<Vec<u8>>,
) -> Result<SourceValue<u16>, MediaError> {
    let bytes = match bytes {
        SourceValue::Present(bytes) => match bytes.as_slice() {
            [] | [_] => WordFact::Missing,
            [low, high] => WordFact::Present(u16::from_le_bytes([*low, *high])),
            _ => return Err(invalid_bytes()),
        },
        SourceValue::Absent
        | SourceValue::Null
        | SourceValue::Invalid
        | SourceValue::Unsupported => WordFact::Missing,
        SourceValue::Unreadable | _ => WordFact::Unavailable,
    };
    match (word_fact(word), bytes) {
        (WordFact::Present(left), WordFact::Present(right)) if left != right => {
            Err(invalid_bytes())
        }
        (WordFact::Present(_), WordFact::Missing) | (WordFact::Missing, WordFact::Present(_)) => {
            Err(invalid_bytes())
        }
        (WordFact::Present(value), _) | (_, WordFact::Present(value)) => {
            Ok(SourceValue::Present(value))
        }
        (WordFact::Missing, _) | (_, WordFact::Missing) => Ok(SourceValue::Invalid),
        (WordFact::Unavailable, WordFact::Unavailable) => Ok(SourceValue::Unreadable),
    }
}
