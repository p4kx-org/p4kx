// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(crate) use crate::layout_cursor::LayoutCursor;
use crate::source_layout_index::{SourceLayout, SourceLayoutIndex};
use crate::{DataForge, DataForgeError, DataType};

/// One field of a struct as it lies in an instance.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldSlot<'source> {
    pub(crate) name: &'source str,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) data_type: DataType,
    pub(crate) array: bool,
    pub(crate) target: u16,
}

/// A named layout backed by validated original source segments.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Layout<'source> {
    pub(crate) type_name: &'source str,
    source: SourceLayout<'source>,
}

impl<'source> Layout<'source> {
    pub(crate) fn from_index(
        forge: &'source DataForge<'source>,
        index: &'source SourceLayoutIndex,
        struct_index: u32,
    ) -> Result<Self, DataForgeError> {
        let source = index.layout(forge, struct_index)?;
        let row = index.row(struct_index as usize)?;
        Ok(Self {
            type_name: forge.blob.get(row.name)?,
            source,
        })
    }

    pub(crate) const fn field_count(&self) -> u64 {
        self.source.field_count()
    }

    pub(crate) fn cursor(&self) -> Result<LayoutCursor<'source>, DataForgeError> {
        self.source.cursor()
    }
}

/// A reusable borrow of the current DataForge's immutable source-layout index.
#[derive(Debug, Default)]
pub(crate) struct Layouts<'source> {
    source: Option<(&'source DataForge<'source>, &'source SourceLayoutIndex)>,
}

impl<'source> Layouts<'source> {
    pub(crate) fn get(
        &mut self,
        forge: &'source DataForge<'source>,
        struct_index: u32,
    ) -> Result<Layout<'source>, DataForgeError> {
        let index = match self.source {
            Some((source, index)) if std::ptr::eq(source, forge) => index,
            _ => {
                let index = forge.source_layouts()?;
                self.source = Some((forge, index));
                index
            }
        };
        Layout::from_index(forge, index, struct_index)
    }

    pub(crate) const fn clear(&mut self) {
        self.source = None;
    }
}
