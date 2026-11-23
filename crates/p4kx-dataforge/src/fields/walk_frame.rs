// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::build;
use crate::fields::observation::{FieldStatisticsInput, FieldStatisticsSlot};
use crate::fields::part::{Part, Stop};
use crate::fields::{FieldKind, FieldStatisticsRouteTarget};
use crate::layout::{FieldSlot, Layout};
use crate::layout_cursor::LayoutCursor;
use crate::{DataForge, ValueKind};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct Owner {
    pub(super) structure: u32,
    pub(super) instance: u32,
}

pub(super) struct Site {
    pub(super) node: u32,
    pub(super) input: Option<FieldStatisticsInput>,
}

pub(super) enum Action<'source> {
    Field {
        slot: FieldSlot<'source>,
        bytes: &'source [u8],
        site: Site,
        owner: Owner,
    },
    Single {
        slot: FieldSlot<'source>,
        bytes: &'source [u8],
        site: Site,
    },
    Class {
        slot: FieldSlot<'source>,
        element: u32,
        site: Site,
    },
}

impl Action<'_> {
    pub(super) fn selector(&self) -> Option<FieldStatisticsInput> {
        let site = match self {
            Self::Field { site, .. } | Self::Single { site, .. } | Self::Class { site, .. } => site,
        };
        site.input.clone()
    }
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
    observe_lineage: bool,
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
            observe_lineage: true,
        })
    }

    fn next(
        &mut self,
        forge: &DataForge<'source>,
        part: &mut Part<'_, 'source>,
    ) -> Result<Option<Action<'source>>, Stop> {
        let slot = if part.observer.is_some() {
            let parent = self.parent;
            let mut observe = |source| part.observe_layout(parent, source);
            if std::mem::take(&mut self.observe_lineage) {
                self.cursor.observe_lineage(&mut observe)?;
            }
            self.cursor.next_positioned_slot_with(Some(&mut observe))?
        } else {
            self.cursor.next_positioned_slot()?
        };
        let Some((slot, position, property_index)) = slot else {
            return Ok(None);
        };
        let node = part.field_node(self.parent, slot.name)?;
        let bytes = self.bytes.get(slot.start..slot.end).unwrap_or_default();
        let observed = part.observer.is_some().then(|| FieldStatisticsSlot {
            record_index: part.record,
            owner_structure: self.owner.structure,
            owner_instance: self.owner.instance,
            layout_structure: self.type_index,
            slot_position: position,
            property_index,
            kind: FieldKind::new(slot.data_type, slot.array),
            bytes: forge.byte_span(bytes),
            name: forge.original_text_span(slot.name),
        });
        let site = Site {
            node,
            input: observed.map(FieldStatisticsInput::Slot),
        };
        part.consulted(&site);
        Ok(Some(Action::Field {
            slot,
            bytes,
            site,
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
    pub(super) header: Option<FieldStatisticsSlot>,
}

impl<'source> Array<'source> {
    fn next(
        &mut self,
        forge: &DataForge<'source>,
        part: &Part<'_, 'source>,
    ) -> Option<Action<'source>> {
        if self.next == self.count {
            return None;
        }
        let index = self.next;
        let element = self.first + index;
        self.next += 1;
        let bytes = self
            .kind
            .map(|kind| build::element_bytes(forge, kind, element));
        let site = Site {
            node: self.node,
            input: self
                .header
                .as_ref()
                .map(|header| FieldStatisticsInput::ArrayElement {
                    header: header.clone(),
                    index,
                    pool_index: element,
                    bytes: bytes.and_then(|bytes| forge.byte_span(bytes)),
                }),
        };
        part.consulted(&site);
        Some(match bytes {
            Some(bytes) => Action::Single {
                slot: self.slot,
                bytes,
                site,
            },
            None => Action::Class {
                slot: self.slot,
                element,
                site,
            },
        })
    }
}

impl<'source> Frame<'source> {
    pub(super) const fn source_target(&self) -> FieldStatisticsRouteTarget {
        match self {
            Self::Structure(fields) => FieldStatisticsRouteTarget::Instance {
                owner_structure: fields.owner.structure,
                owner_instance: fields.owner.instance,
                layout_structure: fields.type_index,
            },
            Self::Array(array) => FieldStatisticsRouteTarget::Array {
                first: array.first,
                count: array.count,
            },
        }
    }

    pub(super) fn next(
        &mut self,
        forge: &DataForge<'source>,
        part: &mut Part<'_, 'source>,
    ) -> Result<Option<Action<'source>>, Stop> {
        match self {
            Self::Structure(fields) => fields.next(forge, part),
            Self::Array(items) => Ok(items.next(forge, part)),
        }
    }

    pub(super) const fn inline(&self) -> Option<(Owner, u32)> {
        match self {
            Self::Structure(fields) => Some((fields.owner, fields.type_index)),
            Self::Array(_) => None,
        }
    }
}
