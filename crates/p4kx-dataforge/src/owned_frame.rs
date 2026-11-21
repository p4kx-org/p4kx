// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::field_storage::FieldStorage;
use crate::layout::{FieldSlot, Layout, LayoutCursor};
use crate::positioned_record::InstanceSpans;
use crate::{DataForge, DataForgeError, Field, StructValue, Value};

use super::array::Array;
use super::materialization_budget::MaterializationBudget;
use super::positions::Position;
use super::task::{Input, Site, Task};

pub(super) enum Action<'source> {
    Read(Task<'source>),
    Complete,
}

pub(super) struct Completed<'source> {
    pub(super) value: Value<'source>,
    pub(super) inline: Option<(Position, u32)>,
}

pub(super) enum Frame<'source> {
    Structure(Structure<'source>),
    Array(Array<'source>),
}

pub(super) struct Structure<'source> {
    type_name: &'source str,
    cursor: LayoutCursor<'source>,
    field_count: u64,
    bytes: &'source [u8],
    position: Position,
    group: Position,
    struct_index: u32,
    next: u64,
    pending: Option<FieldSlot<'source>>,
    fields: Vec<Field<'source>>,
}

impl<'source> Structure<'source> {
    pub(super) fn new(
        layout: Layout<'source>,
        bytes: &'source [u8],
        position: Position,
        group: Position,
        struct_index: u32,
        budget: &mut MaterializationBudget,
    ) -> Result<Self, DataForgeError> {
        let cursor = layout.cursor()?;
        let field_count = layout.field_count();

        let fields = budget.reserve(field_count, "Record fields")?;
        Ok(Self {
            type_name: layout.type_name,
            cursor,
            field_count,
            bytes,
            position,
            group,
            struct_index,
            next: 0,
            pending: None,
            fields,
        })
    }

    fn next(&mut self) -> Result<Action<'source>, DataForgeError> {
        if self.pending.is_some() {
            return Err(DataForgeError::CanonicalTreeInvariant);
        }
        let Some(slot) = self.cursor.next_slot()? else {
            return if self.next == self.field_count {
                Ok(Action::Complete)
            } else {
                Err(DataForgeError::LayoutInvariant)
            };
        };
        if self.next >= self.field_count {
            return Err(DataForgeError::LayoutInvariant);
        }
        let ordinal = self.next;
        self.next = self.next.checked_add(1).ok_or(DataForgeError::TooLarge {
            part: "Record field position",
        })?;
        self.pending = Some(slot);
        Ok(Action::Read(Task {
            input: Input::Field {
                slot,
                bytes: self.bytes.get(slot.start..slot.end).unwrap_or_default(),
            },
            site: Some(Site {
                parent: self.position,
                ordinal,
            }),
            group: self.group,
        }))
    }

    fn accept(&mut self, value: Value<'source>) -> Result<(), DataForgeError> {
        let slot = self
            .pending
            .take()
            .ok_or(DataForgeError::CanonicalTreeInvariant)?;
        let count = u64::try_from(self.fields.len()).map_err(|_| DataForgeError::TooLarge {
            part: "Record fields",
        })?;
        if count.checked_add(1) != Some(self.next) || self.next > self.field_count {
            return Err(DataForgeError::CanonicalTreeInvariant);
        }
        self.fields.push(Field {
            storage: FieldStorage::from_slot(&slot),
            name: slot.name,
            value,
        });
        Ok(())
    }
}

impl<'source> Frame<'source> {
    pub(super) fn next(
        &mut self,
        forge: &DataForge<'source>,
    ) -> Result<Action<'source>, DataForgeError> {
        match self {
            Self::Structure(frame) => frame.next(),
            Self::Array(frame) => frame.next(forge),
        }
    }

    pub(super) fn accept(&mut self, value: Value<'source>) -> Result<(), DataForgeError> {
        match self {
            Self::Structure(frame) => frame.accept(value),
            Self::Array(frame) => frame.accept(value),
        }
    }

    pub(super) fn finish(
        self,
        instances: Option<&mut InstanceSpans<'source>>,
    ) -> Result<Completed<'source>, DataForgeError> {
        let completed = match self {
            Self::Structure(frame) => {
                if let Some(instances) = instances {
                    instances.insert(&frame.fields, frame.bytes)?;
                }
                Completed {
                    value: Value::Struct(StructValue {
                        type_name: frame.type_name,
                        struct_index: frame.struct_index,
                        fields: frame.fields,
                    }),
                    inline: Some((frame.group, frame.struct_index)),
                }
            }
            Self::Array(frame) => Completed {
                value: frame.finish(),
                inline: None,
            },
        };
        Ok(completed)
    }
}
