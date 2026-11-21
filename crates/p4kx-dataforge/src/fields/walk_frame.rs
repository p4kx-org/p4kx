// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::build;
use crate::fields::part::{Part, Stop};
use crate::layout::{FieldSlot, Layout};
use crate::layout_cursor::LayoutCursor;
use crate::{DataForge, ValueKind};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Owner {
    pub(super) structure: u32,
    pub(super) instance: u32,
}

#[derive(Clone, Copy)]
pub(super) enum Action<'source> {
    Field {
        slot: FieldSlot<'source>,
        bytes: &'source [u8],
        node: u32,
        owner: Owner,
    },
    Single {
        slot: FieldSlot<'source>,
        bytes: &'source [u8],
        node: u32,
    },
    Class {
        slot: FieldSlot<'source>,
        element: u32,
        node: u32,
    },
}

pub(super) enum Frame<'source> {
    Structure(Structure<'source>),
    Array(Array<'source>),
}

pub(super) struct Structure<'source> {
    cursor: LayoutCursor<'source>,
    bytes: &'source [u8],
    pub(super) owner: Owner,
    pub(super) type_index: u32,
    parent: u32,
}

impl<'source> Structure<'source> {
    pub(super) fn new(
        layout: Layout<'source>,
        bytes: &'source [u8],
        parent: u32,
        owner: Owner,
        structure: u32,
    ) -> Result<Self, Stop> {
        Ok(Self {
            cursor: layout.cursor()?,
            bytes,
            owner,
            type_index: structure,
            parent,
        })
    }

    fn next(&mut self, part: &mut Part<'_, 'source>) -> Result<Option<Action<'source>>, Stop> {
        let Some(slot) = self.cursor.next_slot()? else {
            return Ok(None);
        };
        let node = part.field_node(self.parent, slot.name)?;
        Ok(Some(Action::Field {
            slot,
            bytes: self.bytes.get(slot.start..slot.end).unwrap_or_default(),
            node,
            owner: self.owner,
        }))
    }
}

pub(super) struct Array<'source> {
    pub(super) slot: FieldSlot<'source>,
    pub(super) kind: Option<ValueKind>,
    pub(super) count: u32,
    pub(super) first: u32,
    pub(super) node: u32,
    pub(super) next: u32,
}

impl<'source> Array<'source> {
    fn next(&mut self, forge: &DataForge<'source>) -> Option<Action<'source>> {
        if self.next == self.count {
            return None;
        }
        let element = self.first + self.next;
        self.next += 1;
        Some(match self.kind {
            Some(kind) => Action::Single {
                slot: self.slot,
                bytes: build::element_bytes(forge, kind, element),
                node: self.node,
            },
            None => Action::Class {
                slot: self.slot,
                element,
                node: self.node,
            },
        })
    }
}

impl<'source> Frame<'source> {
    pub(super) fn next(
        &mut self,
        forge: &DataForge<'source>,
        part: &mut Part<'_, 'source>,
    ) -> Result<Option<Action<'source>>, Stop> {
        match self {
            Self::Structure(fields) => fields.next(part),
            Self::Array(items) => Ok(items.next(forge)),
        }
    }

    pub(super) const fn inline(&self) -> Option<(Owner, u32)> {
        match self {
            Self::Structure(fields) => Some((fields.owner, fields.type_index)),
            Self::Array(_) => None,
        }
    }
}
