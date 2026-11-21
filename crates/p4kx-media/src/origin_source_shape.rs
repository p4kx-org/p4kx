// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use p4kx_extraction::{SourceDeclarationKind, SourceRecordFormat};
use serde::Deserialize;
use serde::de::{Deserializer, SeqAccess, Visitor};

use crate::origin_decimal;

/// A validated decimal coordinate without original-input authority.
pub(crate) struct DecimalCoordinate;

impl<'de> Deserialize<'de> for DecimalCoordinate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        origin_decimal::deserialize(deserializer).map(|_| Self)
    }
}

/// Distinguishes full numeric coordinates from minimal decimal proof coordinates.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryIndex {
    Numeric,
    Decimal,
}

impl<'de> Deserialize<'de> for EntryIndex {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(IndexVisitor)
    }
}

struct IndexVisitor;

impl Visitor<'_> for IndexVisitor {
    type Value = EntryIndex;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an unsigned entry index or canonical decimal proof coordinate")
    }

    fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<EntryIndex, E> {
        usize::try_from(value).map_err(E::custom)?;
        Ok(EntryIndex::Numeric)
    }

    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<EntryIndex, E> {
        origin_decimal::parse(value).map_err(E::custom)?;
        Ok(EntryIndex::Decimal)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ProofDetails {
    CaptureLimited,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RecordFormat {
    ZipCentralDirectory,
    P4kV2,
    Unknown,
}

impl RecordFormat {
    pub(crate) const fn shared(&self) -> SourceRecordFormat {
        match self {
            Self::ZipCentralDirectory => SourceRecordFormat::ZipCentralDirectory,
            Self::P4kV2 => SourceRecordFormat::P4kV2,
            Self::Unknown => SourceRecordFormat::Unknown,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct RecordRange {
    #[serde(rename = "offset")]
    _offset: DecimalCoordinate,
    #[serde(rename = "length")]
    _length: DecimalCoordinate,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum DeclarationKind {
    CigExtraBlock,
    P4kV2Field,
    Unknown,
}
impl DeclarationKind {
    pub(crate) const fn shared(&self) -> SourceDeclarationKind {
        match self {
            Self::CigExtraBlock => SourceDeclarationKind::CigExtraBlock,
            Self::P4kV2Field => SourceDeclarationKind::P4kV2Field,
            Self::Unknown => SourceDeclarationKind::Unknown,
        }
    }
}

/// A checked coordinate string whose contents are not retained as authority.
pub(crate) struct CoordinateText;

impl<'de> Deserialize<'de> for CoordinateText {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(TextVisitor)
    }
}

struct TextVisitor;

impl Visitor<'_> for TextVisitor {
    type Value = CoordinateText;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one descriptive coordinate string")
    }

    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<CoordinateText, E> {
        Ok(CoordinateText)
    }
}

/// Checked decimal ordinals without copied coordinate storage.
pub(crate) struct CoordinateIndices;

impl<'de> Deserialize<'de> for CoordinateIndices {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(IndicesVisitor)
    }
}

struct IndicesVisitor;

impl<'de> Visitor<'de> for IndicesVisitor {
    type Value = CoordinateIndices;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("canonical decimal coordinate ordinals")
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut sequence: S) -> Result<CoordinateIndices, S::Error> {
        let mut coordinate = sequence.next_element::<DecimalCoordinate>()?;
        while coordinate.is_some() {
            coordinate = sequence.next_element::<DecimalCoordinate>()?;
        }
        Ok(CoordinateIndices)
    }
}
