// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::positioned_record::ValueSlot;
use crate::{DataType, PositionedRecord, RecordJsonBytes, RecordJsonSource, Value};

/// Local original observations for one property name, storage declaration or decoded value.
#[derive(Debug)]
pub struct PositionedSources {
    sources: Vec<RecordJsonSource<'static>>,
    complete: bool,
}

impl PositionedSources {
    pub(crate) const fn new(sources: Vec<RecordJsonSource<'static>>, complete: bool) -> Self {
        Self { sources, complete }
    }

    /// Returns the observed original declaration and byte coordinates.
    #[must_use]
    pub fn sources(&self) -> &[RecordJsonSource<'static>] {
        &self.sources
    }

    /// Returns whether every required local coordinate was observed.
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.complete
    }

    pub(crate) fn push(&mut self, source: RecordJsonSource<'static>) {
        self.sources.push(source);
    }

    pub(crate) fn require(&mut self, source: Option<RecordJsonSource<'static>>) {
        self.complete &= source.is_some();
        self.sources.extend(source);
    }

    pub(crate) fn append(&mut self, sources: Option<Self>) {
        if let Some(sources) = sources {
            self.complete &= sources.complete;
            self.sources.extend(sources.sources);
        } else {
            self.complete = false;
        }
    }

    pub(crate) const fn incomplete(&mut self) {
        self.complete = false;
    }

    pub(crate) fn into_parts(self) -> (Vec<RecordJsonSource<'static>>, bool) {
        (self.sources, self.complete)
    }
}

impl PositionedRecord<'_> {
    pub(crate) fn describe_storage(&self, slot: ValueSlot<'_>) -> PositionedSources {
        let mut sources = vec![property_storage(slot)];
        let kind = if slot.array_header.is_some() {
            RecordJsonBytes::ArrayElement
        } else if slot.array {
            RecordJsonBytes::ArrayHeader
        } else {
            RecordJsonBytes::Slot
        };
        sources.extend(self.describe_bytes(slot.array_header, RecordJsonBytes::ArrayHeader));
        let original = self.describe_bytes(slot.bytes, kind);
        let complete = original.is_some() || slot.bytes == Some(&[]);
        sources.extend(original);
        PositionedSources::new(sources, complete)
    }

    pub(crate) fn describe_container_selection(
        &self,
        slot: ValueSlot<'_>,
        value: &Value<'_>,
    ) -> Option<PositionedSources> {
        match value {
            Value::Struct(_) => Some(self.describe_structure_selection(slot)),
            Value::Array(_) => {
                let mut sources = PositionedSources::new(vec![property_storage(slot)], true);
                sources.require(self.describe_bytes(slot.bytes, RecordJsonBytes::ArrayHeader));
                Some(sources)
            }
            _ => None,
        }
    }

    pub(crate) fn describe_structure_selection(&self, slot: ValueSlot<'_>) -> PositionedSources {
        let mut sources = self.describe_selection_header(slot);
        if slot.array && slot.array_header.is_none() {
            sources.incomplete();
        }
        match slot.data_type {
            DataType::Class => {}
            DataType::StrongPointer => {
                sources.require(self.describe_bytes(slot.bytes, RecordJsonBytes::PointerSelector));
            }
            _ => sources.incomplete(),
        }
        sources
    }

    pub(crate) fn describe_selection_header(&self, slot: ValueSlot<'_>) -> PositionedSources {
        let mut sources = PositionedSources::new(vec![property_storage(slot)], true);
        if slot.array_header.is_some() {
            sources.require(self.describe_bytes(slot.array_header, RecordJsonBytes::ArrayHeader));
        }
        sources
    }

    pub(crate) fn describe_bytes(
        &self,
        bytes: Option<&[u8]>,
        kind: RecordJsonBytes,
    ) -> Option<RecordJsonSource<'static>> {
        let span = self.forge.byte_span(bytes?)?;
        Some(RecordJsonSource::Bytes {
            start: span.start,
            end: span.end,
            kind,
        })
    }

    pub(crate) fn describe_text(&self, text: &str) -> Option<RecordJsonSource<'static>> {
        if !text.is_empty() {
            return self.describe_bytes(Some(text.as_bytes()), RecordJsonBytes::Text);
        }
        let start = text
            .as_ptr()
            .addr()
            .checked_sub(self.forge.bytes.as_ptr().addr())?;
        let end = start.checked_add(1)?;
        if self.forge.bytes.get(start..end) != Some(&[0]) {
            return None;
        }
        self.describe_bytes(self.forge.bytes.get(start..end), RecordJsonBytes::Text)
    }
}

const fn property_storage(slot: ValueSlot<'_>) -> RecordJsonSource<'static> {
    RecordJsonSource::PropertyStorage {
        index: slot.property,
        target_used: matches!(slot.data_type, DataType::Class | DataType::Enum),
    }
}
