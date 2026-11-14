// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use super::observation::Observation;
use crate::positioned_record::ValueSlot;
use crate::{PositionedRecord, Value};

/// One step through the physical canonical Record field or array order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordJsonStep {
    /// Inherited-first field ordinal of the current structure.
    Field(usize),
    /// Original slot of the current array.
    Element(u32),
}

pub(super) fn selected_value<'tree, 'source>(
    input: &'tree PositionedRecord<'source>,
    steps: &[RecordJsonStep],
    observation: &mut Observation<'_, 'source>,
) -> io::Result<(&'tree Value<'source>, ValueSlot<'source>)> {
    let Some((RecordJsonStep::Field(first), rest)) = steps.split_first() else {
        return Err(invalid());
    };
    let root = input.tree().root();
    let mut value = root.fields().get(*first).ok_or_else(invalid)?.value();
    let mut slot = *input
        .fields(root.struct_index(), root.fields())
        .map_err(io::Error::other)?
        .get(*first)
        .ok_or_else(invalid)?;
    for step in rest {
        observation.select(slot);
        (value, slot) = match (step, value) {
            (RecordJsonStep::Field(index), Value::Struct(structure)) => (
                structure.fields().get(*index).ok_or_else(invalid)?.value(),
                *input
                    .fields(structure.struct_index(), structure.fields())
                    .map_err(io::Error::other)?
                    .get(*index)
                    .ok_or_else(invalid)?,
            ),
            (RecordJsonStep::Element(index), Value::Array(items)) => (
                items.get(*index as usize).ok_or_else(invalid)?,
                input.array_element(slot, *index),
            ),
            _ => return Err(invalid()),
        };
    }
    Ok((value, slot))
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "invalid original Record value path",
    )
}
