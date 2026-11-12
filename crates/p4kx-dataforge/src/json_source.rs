// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

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
    /// Original text selected by a stored offset or value.
    Text,
}
