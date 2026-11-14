// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::DataForgeError;
use crate::rows::{NO_PARENT, StructDef};
use crate::source_layout_allocation;
use crate::source_layout_properties::SourceProperties;

/// Validated coordinates of one original own-property range.
#[derive(Debug, Clone)]
pub(crate) struct LayoutSummary {
    pub(crate) parent: Option<usize>,
    pub(crate) fields: Range<usize>,
    pub(crate) field_count: u64,
    pub(crate) inherited_count: u64,
    pub(crate) inherited_bytes: u64,
    pub(crate) size: u64,
    pub(crate) depth: usize,
}

/// An invalid layout, with cycles named only at the requested struct boundary.
#[derive(Debug, Clone)]
pub(crate) enum LayoutFailure {
    Source(DataForgeError),
    Cycle,
}

impl From<DataForgeError> for LayoutFailure {
    fn from(error: DataForgeError) -> Self {
        Self::Source(error)
    }
}

#[derive(Debug)]
enum State {
    Unseen,
    Visiting,
    Done(Result<LayoutSummary, LayoutFailure>),
}

/// Validates each parent chain and source range once, with flat traversal state.
pub(crate) fn validate(
    rows: &[StructDef],
    properties: &SourceProperties,
) -> Result<Box<[Result<LayoutSummary, LayoutFailure>]>, DataForgeError> {
    let mut states = Vec::new();
    let mut path = Vec::new();
    source_layout_allocation::reserve(&mut states, rows.len(), "source layout states")?;
    source_layout_allocation::reserve(&mut path, rows.len(), "source layout ancestry")?;
    states.resize_with(rows.len(), || State::Unseen);
    for first in 0..rows.len() {
        if !matches!(states.get(first), Some(State::Unseen)) {
            continue;
        }
        discover(rows, &mut states, &mut path, first)?;
        while let Some(index) = path.pop() {
            let row = *rows.get(index).ok_or(DataForgeError::LayoutInvariant)?;
            let outcome = summarize(index, row, properties, &states);
            *states
                .get_mut(index)
                .ok_or(DataForgeError::LayoutInvariant)? = State::Done(outcome);
        }
    }
    let mut layouts = Vec::new();
    source_layout_allocation::reserve(&mut layouts, rows.len(), "source layout summaries")?;
    for state in states {
        match state {
            State::Done(layout) => layouts.push(layout),
            State::Unseen | State::Visiting => return Err(DataForgeError::LayoutInvariant),
        }
    }
    Ok(layouts.into_boxed_slice())
}

fn discover(
    rows: &[StructDef],
    states: &mut [State],
    path: &mut Vec<usize>,
    first: usize,
) -> Result<(), DataForgeError> {
    let mut current = first;
    loop {
        match states.get(current).ok_or(DataForgeError::LayoutInvariant)? {
            State::Done(_) => return Ok(()),
            State::Visiting => {
                for index in path.drain(..) {
                    *states
                        .get_mut(index)
                        .ok_or(DataForgeError::LayoutInvariant)? =
                        State::Done(Err(LayoutFailure::Cycle));
                }
                return Ok(());
            }
            State::Unseen => {}
        }
        *states
            .get_mut(current)
            .ok_or(DataForgeError::LayoutInvariant)? = State::Visiting;
        path.push(current);
        let row = rows.get(current).ok_or(DataForgeError::LayoutInvariant)?;
        if row.parent == NO_PARENT {
            return Ok(());
        }
        current = usize::try_from(row.parent).map_err(|_| DataForgeError::LayoutInvariant)?;
    }
}

fn summarize(
    index: usize,
    row: StructDef,
    properties: &SourceProperties,
    states: &[State],
) -> Result<LayoutSummary, LayoutFailure> {
    let parent = if row.parent == NO_PARENT {
        None
    } else {
        Some(usize::try_from(row.parent).map_err(|_| DataForgeError::LayoutInvariant)?)
    };
    let inherited = match parent {
        None => None,
        Some(index) => match states.get(index) {
            Some(State::Done(Ok(summary))) => Some(summary),
            Some(State::Done(Err(error))) => return Err(error.clone()),
            _ => return Err(DataForgeError::LayoutInvariant.into()),
        },
    };
    let inherited_count = inherited.map_or(0, |summary| summary.field_count);
    let inherited_bytes = inherited.map_or(0, |summary| summary.size);
    let (fields, bytes) = properties.range(row)?;
    let size = inherited_bytes
        .checked_add(bytes)
        .ok_or(DataForgeError::TooLarge { part: "struct" })?;
    if size != u64::from(row.size) {
        return Err(DataForgeError::StructLayout {
            struct_index: u32::try_from(index).map_err(|_| DataForgeError::LayoutInvariant)?,
            declared: row.size,
            fields: size,
        }
        .into());
    }
    let field_count = inherited_count
        .checked_add(u64::from(row.property_count))
        .ok_or(DataForgeError::TooLarge {
            part: "struct fields",
        })?;
    Ok(LayoutSummary {
        parent,
        fields,
        field_count,
        inherited_count,
        inherited_bytes,
        size,
        depth: inherited
            .map_or(0, |summary| summary.depth)
            .checked_add(1)
            .ok_or(DataForgeError::TooLarge {
                part: "struct ancestry",
            })?,
    })
}
