// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;

use crate::query::sink::Recording;
use crate::query::state::WalkState;
use crate::query::walk::Place;
use crate::scalar::read_scalar;
use crate::{DataForge, DataType, Value, ValueKind};

/// Where a scalar lies in the bytes a path has reached, and how it is read.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Column {
    pub(crate) data_type: DataType,
    pub(crate) target: u16,
    pub(crate) start: u64,
    pub(crate) end: u64,
}

impl Column {
    /// Reads the value from `place`, or the error that its bytes end before the column does.
    pub(crate) fn read<'f>(&self, forge: &'f DataForge<'f>, place: Place<'f>) -> Value<'f> {
        let bytes = usize::try_from(self.start)
            .ok()
            .zip(usize::try_from(self.end).ok())
            .and_then(|(start, end)| place.bytes().get(start..end));
        bytes.map_or_else(
            || Value::Error(place.too_short(forge, self.end)),
            |bytes| read_scalar(forge, self.data_type, self.target, bytes).unwrap_or(Value::Null),
        )
    }
}

/// How a compiled path reads its values from the instance of a Record.
#[derive(Debug, Clone)]
pub(crate) enum Reader {
    /// A scalar at a fixed place in the instance, reached through fields and inline structs only.
    Column(Column),
    /// A path through pointers or array elements.
    Walk(Arc<Walk>),
}

/// The pointer and array steps of a path and the scalar read behind the last of them.
#[derive(Debug)]
pub(crate) struct Walk {
    pub(crate) steps: Box<[Step]>,
    pub(crate) leaf: Column,
    pub(crate) multi: bool,
}

/// One step that leaves the bytes reached so far, at the offset `at` in them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Step {
    /// A strong pointer to an instance of `declared` or of a struct deriving from it.
    Pointer { at: u64, declared: Box<[usize]> },
    /// An array header and the elements it selects.
    Array {
        at: u64,
        elements: Elements,
        select: Select,
    },
    /// A link `->` follows to the instance of `declared` or of a struct deriving from it.
    Follow {
        at: u64,
        link: Link,
        declared: Box<[usize]>,
    },
}

/// What `->` follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Link {
    /// A Reference, to the instance of the Record with its GUID.
    Reference,
    /// A weak pointer, to the instance it names.
    Weak,
}

/// What the elements of an array are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Elements {
    /// Values in the pool of their kind.
    Values(ValueKind),
    /// Instances of the struct at this index.
    Instances(u32),
    /// Strong pointers to instances of `declared` or of a struct deriving from it.
    Pointers(Box<[usize]>),
}

/// Which elements of an array a step selects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Select {
    Index(u32),
    All,
    Type(Box<[usize]>),
}

impl Reader {
    /// Returns the reader with every offset into the Record's instance moved by `base`.
    pub(crate) fn shifted(&self, base: u64) -> Self {
        let shift = |column: Column| Column {
            start: column.start.saturating_add(base),
            end: column.end.saturating_add(base),
            ..column
        };
        match self {
            Self::Column(column) => Self::Column(shift(*column)),
            Self::Walk(walk) => {
                let mut steps = walk.steps.clone();
                match steps.first_mut() {
                    Some(
                        Step::Pointer { at, .. } | Step::Array { at, .. } | Step::Follow { at, .. },
                    ) => {
                        *at = at.saturating_add(base);
                    }
                    None => {}
                }
                Self::Walk(Arc::new(Walk {
                    steps,
                    leaf: walk.leaf,
                    multi: walk.multi,
                }))
            }
        }
    }

    /// Returns the cell of the path in the Record instance at `place`: its value, or for a
    /// multi-valued path every value it reaches as a [`Value::Array`]. Only a multi-valued path
    /// allocates, for its list.
    ///
    /// With `prefix`, the recording of the first steps the path shares with other columns and
    /// their number, the walk takes those steps from the recording; the cell is the same.
    pub(crate) fn cell<'f>(
        &self,
        forge: &'f DataForge<'f>,
        place: Place<'f>,
        state: &mut WalkState,
        prefix: Option<(&Recording<'f>, usize)>,
    ) -> Value<'f> {
        let walk = match self {
            Self::Column(column) => return column.read(forge, place),
            Self::Walk(walk) => walk,
        };
        let drive = |state: &mut WalkState, emit: &mut dyn FnMut(Value<'f>) -> ControlFlow<()>| {
            match prefix {
                Some((recording, steps)) => walk.replay(forge, recording, steps, state, emit),
                None => walk.visit(forge, place, state, emit),
            }
        };
        if !walk.multi {
            let mut single = Value::Null;
            let reached = drive(state, &mut |value| {
                single = value;
                ControlFlow::Break(())
            });
            return reached.map_or_else(Value::Error, |()| single);
        }
        let mut values = Vec::new();
        let reached = drive(state, &mut |value| {
            values.push(value);
            ControlFlow::Continue(())
        });
        reached.map_or_else(Value::Error, |()| Value::Array(values.into_boxed_slice()))
    }
}
