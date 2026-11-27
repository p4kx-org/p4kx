// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;
use std::io;

use crate::positioned_record::ValueSlot;
use crate::positioned_reference_members::DanglingReferenceMember;
use crate::{
    Guid, ObservedResolution, PositionedRecord, PositionedReferenceSources, PositionedSources,
    RecordJsonReferenceMember, RecordJsonReferenceSources, RecordJsonUse, RecordJsonUseRole,
    Reference, StructValue, Value,
};

use super::source::{RecordJsonBytes, RecordJsonObserver, RecordJsonRecordField, RecordJsonSource};

pub(super) struct Observation<'observer, 'source> {
    input: &'observer PositionedRecord<'source>,
    observer: &'observer mut dyn RecordJsonObserver,
    pointer: String,
    lengths: Vec<usize>,
    structures: Vec<Vec<ValueSlot<'source>>>,
    selection: PositionedSources,
    nested: Vec<Route>,
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
            selection: PositionedSources::new(Vec::new(), true),
            nested: Vec::new(),
        }
    }

    pub(super) fn select(&mut self, slot: ValueSlot<'source>, value: &Value<'_>) {
        self.selection
            .append(self.input.describe_container_selection(slot, value));
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
        while self.nested.last().is_some_and(|route| route.depth >= depth) {
            self.nested.pop();
        }
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
        let local_selection_tracked = self
            .nested
            .last()
            .is_some_and(|route| self.lengths.len().checked_sub(1) == Some(route.depth));
        let (mut sources, complete) = if local_selection_tracked {
            (Vec::new(), true)
        } else {
            slot.map_or_default(|slot| self.input.describe_structure_selection(slot).into_parts())
        };
        if let Some(record) = record {
            sources.push(RecordJsonSource::Record {
                index: record,
                field: RecordJsonRecordField::Struct,
            });
        }
        sources.push(RecordJsonSource::StructName { index });
        self.emit(&sources, record.is_some() || complete)
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

    pub(super) fn field_wrapper(
        &mut self,
        slot: Option<ValueSlot<'source>>,
        value: &Value<'_>,
    ) -> io::Result<()> {
        let (sources, complete) = self.slot(slot, Some(value));
        self.emit(&sources, complete)
    }

    pub(super) fn value(
        &mut self,
        slot: Option<ValueSlot<'source>>,
        value: Option<&Value<'_>>,
    ) -> io::Result<()> {
        let (sources, complete) = self.slot(slot, value);
        self.emit(&sources, complete)?;
        if let Some(value) = value
            && matches!(value, Value::Struct(_) | Value::Array(_))
        {
            let selection = slot
                .and_then(|slot| self.input.describe_container_selection(slot, value))
                .unwrap_or_else(|| PositionedSources::new(Vec::new(), false));
            self.nested.push(Route {
                depth: self.lengths.len(),
                selection,
            });
        }
        Ok(())
    }

    pub(super) fn reference_sources(
        &self,
        slot: Option<ValueSlot<'source>>,
        reference: &Reference<'_>,
    ) -> Option<PositionedReferenceSources> {
        PositionedReferenceSources::capture(self.input, slot?, reference)
    }

    pub(super) fn reference(
        &mut self,
        sources: Option<&PositionedReferenceSources>,
        member: RecordJsonReferenceMember,
    ) -> io::Result<()> {
        let local = sources.map_or_else(RecordJsonReferenceSources::unavailable, |sources| {
            sources.member(member)
        });
        self.emit_reference(&local)
    }

    pub(super) fn localization(
        &mut self,
        slot: Option<ValueSlot<'source>>,
        key: &str,
        resolution: &ObservedResolution<'_>,
    ) -> io::Result<()> {
        let (sources, local_complete) = self.slot(slot, Some(&Value::Locale(key)));
        let mut selectors = Vec::new();
        let complete =
            self.append_routes(&mut selectors, RecordJsonUse::selection) && local_complete;
        selectors.extend(sources.into_iter().map(RecordJsonUse::selection));
        self.observer
            .localization_observed(&self.pointer, key, resolution, &selectors, complete)
    }

    pub(super) fn tag(
        &mut self,
        sources: Option<&PositionedReferenceSources>,
        guid: Guid,
        path: &str,
    ) -> io::Result<()> {
        let local = sources.map_or_else(RecordJsonReferenceSources::unavailable, |sources| {
            sources.member(RecordJsonReferenceMember::Guid)
        });
        let mut selectors = Vec::new();
        let complete =
            self.append_routes(&mut selectors, RecordJsonUse::selection) && local.complete();
        selectors.extend(local.sources().iter().map(|used| {
            let role = match used.role {
                RecordJsonUseRole::Payload => RecordJsonUseRole::Selector,
                role => role,
            };
            RecordJsonUse::new(used.source, role)
        }));
        self.observer
            .tag(&self.pointer, guid, path, &selectors, complete)
    }

    pub(super) fn dangling_reference(
        &mut self,
        slot: Option<ValueSlot<'source>>,
        member: DanglingReferenceMember,
    ) -> io::Result<()> {
        let local = RecordJsonReferenceSources::dangling(self.input, slot, member);
        self.emit_reference(&local)
    }

    fn emit_reference(&mut self, local: &RecordJsonReferenceSources) -> io::Result<()> {
        if self.selection.sources().is_empty() && self.nested.is_empty() {
            return self.observer.reference_value(
                &self.pointer,
                local.sources(),
                local.complete() && self.selection.complete(),
            );
        }
        let mut uses = Vec::new();
        let complete = self.append_routes(&mut uses, RecordJsonUse::selection) && local.complete();
        uses.extend_from_slice(local.sources());
        self.observer
            .reference_value(&self.pointer, &uses, complete)
    }

    fn slot(
        &self,
        slot: Option<ValueSlot<'source>>,
        value: Option<&Value<'_>>,
    ) -> (Vec<RecordJsonSource<'static>>, bool) {
        let Some(slot) = slot else {
            return (Vec::new(), false);
        };
        let (mut sources, complete) = self.input.describe_storage(slot).into_parts();
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
        self.input.describe_bytes(bytes, kind)
    }

    fn emit(&mut self, sources: &[RecordJsonSource<'_>], complete: bool) -> io::Result<()> {
        if self.selection.sources().is_empty() && self.nested.is_empty() {
            return self.observer.value(
                &self.pointer,
                sources,
                complete && self.selection.complete(),
            );
        }
        let mut inputs = Vec::new();
        let complete = self.append_routes(&mut inputs, |source| source) && complete;
        inputs.extend_from_slice(sources);
        self.observer.value(&self.pointer, &inputs, complete)
    }

    fn append_routes<T>(
        &self,
        inputs: &mut Vec<T>,
        use_source: impl Fn(RecordJsonSource<'static>) -> T,
    ) -> bool {
        let selections = std::iter::once(&self.selection)
            .chain(self.nested.iter().map(|route| &route.selection));
        let mut complete = true;
        for selection in selections {
            complete &= selection.complete();
            inputs.extend(selection.sources().iter().copied().map(&use_source));
        }
        complete
    }
}

struct Route {
    depth: usize,
    selection: PositionedSources,
}
