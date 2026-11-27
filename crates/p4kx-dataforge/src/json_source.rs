// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use crate::{Guid, ObservedResolution, Resolution};

/// Observes original inputs at addresses emitted by the canonical Record JSON writer.
pub trait RecordJsonObserver {
    /// Receives one emitted value's JSON Pointer and its original contributors.
    ///
    /// # Errors
    /// Returns an error to stop emission at this value.
    fn value(
        &mut self,
        pointer: &str,
        sources: &[RecordJsonSource<'_>],
        complete: bool,
    ) -> io::Result<()>;

    /// Receives an emitted Reference member's original contributors and their roles.
    ///
    /// # Errors
    /// Returns an error to stop emission at this value.
    fn reference_value(
        &mut self,
        pointer: &str,
        sources: &[RecordJsonUse],
        complete: bool,
    ) -> io::Result<()> {
        let coordinates: Vec<_> = sources.iter().map(|input| input.source).collect();
        self.value(pointer, &coordinates, complete)
    }

    /// Receives the actual localisation decision and its original Record selectors.
    ///
    /// # Errors
    /// Returns an error to stop emission at this value.
    fn localization(
        &mut self,
        pointer: &str,
        _key: &str,
        _resolution: &Resolution<'_>,
        selectors: &[RecordJsonUse],
        _selectors_complete: bool,
    ) -> io::Result<()> {
        self.reference_value(pointer, selectors, false)
    }

    /// Receives a localisation decision, its resolver input and its original Record selectors.
    ///
    /// # Errors
    /// Returns an error to stop emission at this value.
    fn localization_observed(
        &mut self,
        pointer: &str,
        key: &str,
        observed: &ObservedResolution<'_>,
        selectors: &[RecordJsonUse],
        selectors_complete: bool,
    ) -> io::Result<()> {
        self.localization(
            pointer,
            key,
            observed.resolution(),
            selectors,
            selectors_complete,
        )
    }

    /// Receives the chosen tag path, its target GUID and the original selecting Reference.
    ///
    /// # Errors
    /// Returns an error to stop emission at this value.
    fn tag(
        &mut self,
        pointer: &str,
        _guid: Guid,
        _path: &str,
        selectors: &[RecordJsonUse],
        _selectors_complete: bool,
    ) -> io::Result<()> {
        self.reference_value(pointer, selectors, false)
    }
}

/// How an original input contributes to an observed value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordJsonUseRole {
    /// Supplies the emitted data.
    Payload,
    /// Selects the original field, element, or target.
    Selector,
    /// Determines how to interpret or render original data.
    Conversion,
}

/// A source coordinate and its contribution to an observed value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecordJsonUse {
    /// Original source coordinate.
    pub source: RecordJsonSource<'static>,
    /// Role of that coordinate for this member.
    pub role: RecordJsonUseRole,
}

impl RecordJsonUse {
    /// Pairs an observed original coordinate with its role in the emitted value.
    #[must_use]
    pub const fn new(source: RecordJsonSource<'static>, role: RecordJsonUseRole) -> Self {
        Self { source, role }
    }

    pub(crate) const fn selection(source: RecordJsonSource<'static>) -> Self {
        let role = match source {
            RecordJsonSource::PropertyStorage { .. } => RecordJsonUseRole::Conversion,
            _ => RecordJsonUseRole::Selector,
        };
        Self { source, role }
    }
}

/// One observed input that contributes to an emitted Record JSON value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordJsonSource<'source> {
    /// An original field of a physical Record table row.
    Record {
        /// Original Record index.
        index: u32,
        /// The table field used by the writer.
        field: RecordJsonRecordField,
    },
    /// The name of an original struct definition.
    StructName {
        /// Original struct index.
        index: u32,
    },
    /// The authored name of a physical property definition.
    PropertyName {
        /// Original property index, including inherited declarations.
        index: usize,
    },
    /// The original property words that determine an instance layout.
    PropertyStorage {
        /// Original property index, including inherited declarations.
        index: usize,
        /// Whether the decoder uses the declared target in addition to type and conversion.
        target_used: bool,
    },
    /// The original field and array positions leading to a selected owned instance.
    InstancePath {
        /// Original Record index.
        record: u32,
        /// Original positions in inherited-first order.
        ordinals: &'source [u64],
    },
    /// A half-open range within the original decoded DataForge bytes.
    Bytes {
        /// Inclusive byte offset.
        start: u64,
        /// Exclusive byte offset.
        end: u64,
        /// The interpretation of these bytes.
        kind: RecordJsonBytes,
    },
}

/// A Record table field used by the JSON writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordJsonRecordField {
    /// Record GUID.
    Guid,
    /// Record file name.
    FileName,
    /// Authored Record name.
    Name,
    /// Authored Record tag.
    Tag,
    /// Selected struct.
    Struct,
}

/// A byte range interpreted by the canonical Record reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordJsonBytes {
    /// Direct scalar, pointer or inline structure storage.
    Slot,
    /// Original array count and first element words.
    ArrayHeader,
    /// Original selected element in a pool or instance block.
    ArrayElement,
    /// Original struct and instance words selecting a strong pointer target.
    PointerSelector,
    /// Original text selected by a stored offset or value.
    Text,
}
