// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use crate::layout::FieldSlot;
use crate::layout_source::LayoutSource;
use crate::slot::Slot;
use crate::source_layout_allocation;
use crate::source_layout_index::SourceLayout;
use crate::source_layout_properties::SourceField;
use crate::{DataForge, DataForgeError};

/// A logical field occurrence at its original inherited-first position and byte coordinates.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LayoutField<'source> {
    pub(crate) name: &'source str,
    pub(crate) position: u64,
    pub(crate) slot: Slot,
    pub(crate) start: u64,
    pub(crate) end: u64,
}

#[derive(Debug)]
struct Segment<'source> {
    fields: slice::Iter<'source, SourceField>,
    structure: usize,
    position: u64,
    offset: u64,
}

/// Iterates borrowed validated own-field segments without storing inherited prefixes.
#[derive(Debug)]
pub(crate) struct LayoutCursor<'source> {
    forge: &'source DataForge<'source>,
    segments: Vec<Segment<'source>>,
    current: Option<Segment<'source>>,
}

impl<'source> LayoutCursor<'source> {
    pub(crate) fn new(layout: SourceLayout<'source>) -> Result<Self, DataForgeError> {
        let mut segments = Vec::new();
        source_layout_allocation::reserve(
            &mut segments,
            layout.summary.depth,
            "source layout cursor",
        )?;
        let mut summary = layout.summary;
        let mut structure = layout.struct_index;
        loop {
            let fields = layout
                .index
                .properties
                .fields
                .get(summary.fields.clone())
                .ok_or(DataForgeError::LayoutInvariant)?;
            segments.push(Segment {
                fields: fields.iter(),
                structure,
                position: summary.inherited_count,
                offset: summary.inherited_bytes,
            });
            match summary.parent {
                Some(parent) => {
                    structure = parent;
                    summary = layout.index.summary(parent, 0)?;
                }
                None => break,
            }
        }
        Ok(Self {
            forge: layout.forge,
            segments,
            current: None,
        })
    }

    pub(crate) fn next_slot(&mut self) -> Result<Option<FieldSlot<'source>>, DataForgeError> {
        self.next_positioned_slot()
            .map(|slot| slot.map(|(slot, _, _)| slot))
    }

    /// Reports the actual Struct rows of the prepared, unconsumed cursor.
    pub(crate) fn observe_lineage(
        &self,
        mut observe: impl FnMut(LayoutSource),
    ) -> Result<(), DataForgeError> {
        for segment in self.segments.iter().rev() {
            let index = segment.structure;
            let definition = self
                .forge
                .structs
                .get(index)
                .ok_or(DataForgeError::LayoutInvariant)?;
            observe(LayoutSource::Structure {
                index,
                parent: definition.parent,
                first_property: definition.first_property,
                property_count: definition.property_count,
                size: definition.size,
            });
        }
        Ok(())
    }

    /// Returns the slot and its original inherited-first position and property definition.
    pub(crate) fn next_positioned_slot(
        &mut self,
    ) -> Result<Option<(FieldSlot<'source>, u64, usize)>, DataForgeError> {
        self.next_positioned_slot_with(None)
    }

    pub(crate) fn next_positioned_slot_with(
        &mut self,
        observe: Option<&mut dyn FnMut(LayoutSource)>,
    ) -> Result<Option<(FieldSlot<'source>, u64, usize)>, DataForgeError> {
        self.next_with(observe)
            .map(|field| {
                let field = field?;
                let slot = FieldSlot {
                    name: field.name,
                    start: usize::try_from(field.start)
                        .map_err(|_| DataForgeError::TooLarge { part: "struct" })?,
                    end: usize::try_from(field.end)
                        .map_err(|_| DataForgeError::TooLarge { part: "struct" })?,
                    data_type: field.slot.data_type(),
                    array: field.slot.is_array(),
                    target: field.slot.target(),
                };
                Ok((slot, field.position, field.slot.property_index()))
            })
            .transpose()
    }
}

impl<'source> LayoutCursor<'source> {
    pub(crate) fn next_with(
        &mut self,
        observe: Option<&mut dyn FnMut(LayoutSource)>,
    ) -> Option<Result<LayoutField<'source>, DataForgeError>> {
        loop {
            if let Some(segment) = &mut self.current
                && let Some(field) = segment.fields.next()
            {
                let start = segment.offset;
                segment.offset += field.width;
                let position = segment.position;
                segment.position += 1;
                let name = field.slot.name(self.forge);
                if let Some(observe) = observe {
                    let index = field.slot.property_index();
                    observe(match &name {
                        Ok(name) => LayoutSource::Property {
                            index,
                            position,
                            name: self.forge.original_text_span(name),
                        },
                        Err(_) => LayoutSource::UnreadablePropertyName { index },
                    });
                }
                return Some(name.map(|name| LayoutField {
                    name,
                    position,
                    slot: field.slot,
                    start,
                    end: segment.offset,
                }));
            }
            self.current = self.segments.pop();
            self.current.as_ref()?;
        }
    }
}

impl<'source> Iterator for LayoutCursor<'source> {
    type Item = Result<LayoutField<'source>, DataForgeError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_with(None)
    }
}
