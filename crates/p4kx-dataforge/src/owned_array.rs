// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::layout::FieldSlot;
use crate::{DataForge, DataForgeError, DataType, Value, ValueError, ValueKind};

use super::bytes;
use super::frame::Action;
use super::materialization_budget::MaterializationBudget;
use super::positions::{Position, Positions};
use super::task::{Input, Site, Task};

pub(super) struct Array<'source> {
    slot: FieldSlot<'source>,
    kind: Option<ValueKind>,
    first: u32,
    count: u32,
    next: u32,
    position: Option<Position>,
    group: Position,
    items: Vec<Value<'source>>,
}

impl<'source> Array<'source> {
    pub(super) fn new(
        forge: &DataForge<'source>,
        slot: FieldSlot<'source>,
        stored: &[u8],
        task: Task<'source>,
        positions: &mut Positions,
        budget: &mut MaterializationBudget,
    ) -> Result<Result<Self, ValueError>, DataForgeError> {
        let [count, first] = bytes::words(stored);
        let kind = ValueKind::of(slot.data_type);
        let (len, pool) = match kind {
            Some(kind) => (forge.value_count(kind) as u64, kind.name()),
            None => match forge.instance_counts.get(usize::from(slot.target)) {
                Some(count) => (u64::from(*count), "instance block"),
                None => return Ok(Err(ValueError::UnknownStruct { index: slot.target })),
            },
        };
        if u64::from(first) + u64::from(count) > len {
            return Ok(Err(ValueError::ArrayOutOfRange {
                first,
                count,
                len,
                pool,
            }));
        }
        let items = budget.reserve(u64::from(count), "Record array")?;
        let position = if matches!(
            slot.data_type,
            DataType::Class | DataType::StrongPointer | DataType::WeakPointer
        ) {
            Some(task.position(positions)?)
        } else {
            None
        };

        Ok(Ok(Self {
            slot,
            kind,
            first,
            count,
            next: 0,
            position,
            group: task.group,
            items,
        }))
    }

    pub(super) fn next(
        &mut self,
        forge: &DataForge<'source>,
    ) -> Result<Action<'source>, DataForgeError> {
        if self.items.len() != self.next as usize {
            return Err(DataForgeError::CanonicalTreeInvariant);
        }
        if self.next == self.count {
            return Ok(Action::Complete);
        }
        let ordinal = self.next;
        self.next += 1;
        let element = self
            .first
            .checked_add(ordinal)
            .ok_or(DataForgeError::CanonicalTreeInvariant)?;
        let input = match self.kind {
            Some(kind) => Input::Single {
                data_type: self.slot.data_type,
                target: self.slot.target,
                bytes: bytes::element_bytes(forge, kind, element),
            },
            None => Input::Owned {
                struct_index: u32::from(self.slot.target),
                instance_index: element,
            },
        };
        Ok(Action::Read(Task {
            input,
            site: self.position.map(|parent| Site {
                parent,
                ordinal: u64::from(ordinal),
            }),
            group: self.group,
        }))
    }

    pub(super) fn accept(&mut self, value: Value<'source>) -> Result<(), DataForgeError> {
        if self.items.len().checked_add(1) != Some(self.next as usize) {
            return Err(DataForgeError::CanonicalTreeInvariant);
        }
        self.items.push(value);
        Ok(())
    }

    pub(super) fn finish(self) -> Value<'source> {
        Value::Array(self.items.into_boxed_slice())
    }
}
