// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use crate::build;
use crate::layout::Layout;
use crate::source_layout_allocation;
use crate::{DataForge, DataForgeError, DataType, Field, RecordTree, ValueKind};

/// A canonical Record paired with the original instance slices admitted by its decoder.
#[derive(Debug)]
pub struct PositionedRecord<'source> {
    tree: RecordTree<'source>,
    pub(crate) forge: &'source DataForge<'source>,
    instances: InstanceSpans<'source>,
}

impl<'source> PositionedRecord<'source> {
    pub(crate) const fn new(
        tree: RecordTree<'source>,
        forge: &'source DataForge<'source>,
        instances: InstanceSpans<'source>,
    ) -> Self {
        Self {
            tree,
            forge,
            instances,
        }
    }

    /// Returns the paired immutable canonical tree.
    #[must_use]
    pub const fn tree(&self) -> &RecordTree<'source> {
        &self.tree
    }

    pub(crate) fn fields(
        &self,
        struct_index: u32,
        fields: &[Field<'source>],
    ) -> Result<Vec<ValueSlot<'source>>, DataForgeError> {
        if fields.is_empty() {
            return Ok(Vec::new());
        }
        let bytes = self
            .instances
            .get(fields)
            .ok_or(DataForgeError::CanonicalTreeInvariant)?;
        let mut slots = Vec::new();
        source_layout_allocation::reserve(
            &mut slots,
            fields.len(),
            "positioned Record field slots",
        )?;
        let layout = Layout::from_index(self.forge, self.forge.source_layouts()?, struct_index)?;
        for field in layout.cursor()? {
            let field = field?;
            let start =
                usize::try_from(field.start).map_err(|_| DataForgeError::LayoutInvariant)?;
            let end = usize::try_from(field.end).map_err(|_| DataForgeError::LayoutInvariant)?;
            slots.push(ValueSlot {
                property: field.slot.property_index(),
                data_type: field.slot.data_type(),
                target: field.slot.target(),
                array: field.slot.is_array(),
                bytes: bytes.get(start..end),
                array_header: None,
            });
        }
        Ok(slots)
    }

    pub(crate) fn array_element(
        &self,
        field: ValueSlot<'source>,
        ordinal: u32,
    ) -> ValueSlot<'source> {
        let bytes = field.bytes.and_then(|header| {
            let [count, first] = build::words(header);
            let element = first.checked_add(ordinal).filter(|_| ordinal < count)?;
            ValueKind::of(field.data_type).map_or_else(
                || self.forge.instance(u32::from(field.target), element),
                |kind| Some(build::element_bytes(self.forge, kind, element)),
            )
        });
        ValueSlot {
            bytes,
            array_header: field.bytes,
            ..field
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ValueSlot<'source> {
    pub(crate) property: usize,
    pub(crate) data_type: DataType,
    target: u16,
    pub(crate) array: bool,
    pub(crate) bytes: Option<&'source [u8]>,
    pub(crate) array_header: Option<&'source [u8]>,
}

#[derive(Debug, Default)]
pub(crate) struct InstanceSpans<'source>(HashMap<usize, &'source [u8]>);

impl<'source> InstanceSpans<'source> {
    pub(crate) fn insert(
        &mut self,
        fields: &[Field<'source>],
        bytes: &'source [u8],
    ) -> Result<(), DataForgeError> {
        const PART: &str = "positioned Record instances";
        if fields.is_empty() {
            return Ok(());
        }
        let elements = self
            .0
            .len()
            .checked_add(1)
            .ok_or(DataForgeError::TooLarge { part: PART })?;
        self.0
            .try_reserve(1)
            .map_err(|_| DataForgeError::LayoutAllocationRefused {
                part: PART,
                elements: elements as u64,
            })?;
        // The decoder and weak-link resolver keep this backing allocation intact.
        self.0.insert(fields.as_ptr().addr(), bytes);
        Ok(())
    }

    fn get(&self, fields: &[Field<'source>]) -> Option<&'source [u8]> {
        self.0.get(&fields.as_ptr().addr()).copied()
    }
}
