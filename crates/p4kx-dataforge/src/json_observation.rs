// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;
use std::io;

use crate::positioned_record::ValueSlot;
use crate::{DataType, PositionedRecord, Reference, StructValue, Value};

use super::source::{RecordJsonBytes, RecordJsonObserver, RecordJsonRecordField, RecordJsonSource};

pub(super) struct Observation<'observer, 'source> {
    input: &'observer PositionedRecord<'source>,
    observer: &'observer mut dyn RecordJsonObserver,
    pointer: String,
    lengths: Vec<usize>,
    structures: Vec<Vec<ValueSlot<'source>>>,
}

impl<'observer, 'source> Observation<'observer, 'source> {
    pub(super) fn new(
        input: &'observer PositionedRecord<'source>,
        observer: &'observer mut dyn RecordJsonObserver,
    ) -> Self {
        Self {
            input,
            observer,
            pointer: String::new(),
            lengths: Vec::new(),
            structures: Vec::new(),
        }
    }

    pub(super) fn structure(
        &mut self,
        depth: usize,
        structure: &StructValue<'source>,
    ) -> io::Result<()> {
        self.structures.truncate(depth);
        self.structures.resize_with(depth, Vec::new);
        self.structures.push(
            self.input
                .fields(structure.struct_index(), structure.fields())
                .map_err(io::Error::other)?,
        );
        Ok(())
    }

    pub(super) fn field(&self, depth: usize, index: usize) -> Option<ValueSlot<'source>> {
        self.structures.get(depth)?.get(index).copied()
    }

    pub(super) fn element(&self, slot: ValueSlot<'source>, index: u32) -> ValueSlot<'source> {
        self.input.array_element(slot, index)
    }

    pub(super) fn key(&mut self, depth: usize, key: &str) {
        self.start_segment(depth);
        for character in key.chars() {
            match character {
                '~' => self.pointer.push_str("~0"),
                '/' => self.pointer.push_str("~1"),
                character => self.pointer.push(character),
            }
        }
        self.lengths.push(self.pointer.len());
    }

    pub(super) fn index(&mut self, depth: usize, index: usize) {
        self.start_segment(depth);
        let _written = write!(&mut self.pointer, "{index}");
        self.lengths.push(self.pointer.len());
    }

    fn start_segment(&mut self, depth: usize) {
        let parent = depth.saturating_sub(1);
        self.pointer.truncate(
            parent
                .checked_sub(1)
                .and_then(|position| self.lengths.get(position))
                .copied()
                .unwrap_or_default(),
        );
        self.lengths.truncate(parent);
        self.pointer.push('/');
    }

    pub(super) fn header(
        &mut self,
        index: u32,
        field: RecordJsonRecordField,
        text: Option<&str>,
    ) -> io::Result<()> {
        let mut sources = vec![RecordJsonSource::Record { index, field }];
        sources.extend(self.bytes(text.map(str::as_bytes), RecordJsonBytes::Text));
        self.emit(&sources, true)
    }

    pub(super) fn structure_name(
        &mut self,
        index: u32,
        record: Option<u32>,
        slot: Option<ValueSlot<'source>>,
    ) -> io::Result<()> {
        let (mut sources, complete) = self.slot(slot, None);
        if let Some(record) = record {
            sources.push(RecordJsonSource::Record {
                index: record,
                field: RecordJsonRecordField::Struct,
            });
        }
        sources.push(RecordJsonSource::StructName { index });
        self.emit(
            &sources,
            record.is_some() || complete || slot.is_some_and(|slot| slot.bytes == Some(&[])),
        )
    }

    pub(super) fn property_name(
        &mut self,
        slot: Option<ValueSlot<'source>>,
        name: &str,
    ) -> io::Result<()> {
        let mut sources = Vec::new();
        sources.extend(slot.map(|slot| RecordJsonSource::PropertyName {
            index: slot.property,
        }));
        sources.extend(self.bytes(Some(name.as_bytes()), RecordJsonBytes::Text));
        self.emit(&sources, slot.is_some())
    }

    pub(super) fn value(
        &mut self,
        slot: Option<ValueSlot<'source>>,
        value: Option<&Value<'_>>,
    ) -> io::Result<()> {
        let (sources, complete) = self.slot(slot, value);
        self.emit(&sources, complete)
    }

    pub(super) fn reference(
        &mut self,
        slot: Option<ValueSlot<'source>>,
        reference: &Reference<'_>,
        member: ReferenceMember,
    ) -> io::Result<()> {
        let (mut sources, complete) = self.slot(slot, None);
        let original = reference.original_target();
        let index = original.record_index();
        match member {
            ReferenceMember::Guid => sources.push(RecordJsonSource::Record {
                index,
                field: RecordJsonRecordField::Guid,
            }),
            ReferenceMember::Path => {
                sources.push(RecordJsonSource::Record {
                    index,
                    field: RecordJsonRecordField::FileName,
                });
                let original_name = original
                    .record()
                    .and_then(|record| record.stored_file_name())
                    .map_err(io::Error::other)?;
                sources.extend(self.bytes(Some(original_name.as_bytes()), RecordJsonBytes::Text));
            }
            ReferenceMember::Type | ReferenceMember::InstancePath => {
                if let Some(ordinals) = original.instance_positions() {
                    sources.push(RecordJsonSource::InstancePath {
                        record: index,
                        ordinals,
                    });
                    sources.extend(self.bytes(
                        Some(reference.type_name().as_bytes()),
                        RecordJsonBytes::Text,
                    ));
                } else {
                    let structure = original
                        .record()
                        .and_then(|record| record.struct_type())
                        .map_err(io::Error::other)?;
                    sources.push(RecordJsonSource::Record {
                        index,
                        field: RecordJsonRecordField::Struct,
                    });
                    sources.push(RecordJsonSource::StructName {
                        index: structure.index(),
                    });
                }
            }
        }
        self.emit(&sources, complete)
    }

    fn slot(
        &self,
        slot: Option<ValueSlot<'source>>,
        value: Option<&Value<'_>>,
    ) -> (Vec<RecordJsonSource<'static>>, bool) {
        let Some(slot) = slot else {
            return (Vec::new(), false);
        };
        let mut sources = vec![RecordJsonSource::PropertyStorage {
            index: slot.property,
            target_used: matches!(slot.data_type, DataType::Class | DataType::Enum),
        }];
        let kind = if slot.array_header.is_some() {
            RecordJsonBytes::ArrayElement
        } else if slot.array {
            RecordJsonBytes::ArrayHeader
        } else {
            RecordJsonBytes::Slot
        };
        sources.extend(self.bytes(slot.array_header, RecordJsonBytes::ArrayHeader));
        let original = self.bytes(slot.bytes, kind);
        let complete = original.is_some() || slot.bytes == Some(&[]);
        sources.extend(original);
        if let Some(Value::String(text) | Value::Locale(text) | Value::Enum(text)) = value {
            sources.extend(self.bytes(Some(text.as_bytes()), RecordJsonBytes::Text));
        }
        (sources, complete)
    }

    fn bytes(
        &self,
        bytes: Option<&[u8]>,
        kind: RecordJsonBytes,
    ) -> Option<RecordJsonSource<'static>> {
        let span = self.input.span(bytes?)?;
        Some(RecordJsonSource::Bytes {
            start: span.start,
            end: span.end,
            kind,
        })
    }

    fn emit(&mut self, sources: &[RecordJsonSource<'_>], complete: bool) -> io::Result<()> {
        self.observer.value(&self.pointer, sources, complete)
    }
}

#[derive(Clone, Copy)]
pub(super) enum ReferenceMember {
    Guid,
    Path,
    Type,
    InstancePath,
}
