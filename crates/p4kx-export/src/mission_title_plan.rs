// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;

use super::graph::{Graph, Node, Segment};
use super::{check_cancel, push};
use crate::CatalogError;

#[derive(Clone, Copy)]
pub(super) enum State {
    Unseen,
    Active,
    Cyclic,
    Finite { bytes: usize, terminal: usize },
}

#[derive(Clone, Copy)]
pub(super) struct Cursor {
    pub(super) node: usize,
    pub(super) next: usize,
}

impl Cursor {
    pub(super) const fn at(node: usize) -> Self {
        Self { node, next: 0 }
    }
}

struct Frame {
    cursor: Cursor,
    bytes: Option<usize>,
}

impl Frame {
    const fn at(node: usize) -> Self {
        Self {
            cursor: Cursor::at(node),
            bytes: Some(0),
        }
    }
}

pub(super) fn analyze(
    graph: &Graph<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Vec<State>, CatalogError> {
    let mut states = Vec::new();
    states
        .try_reserve_exact(graph.len())
        .map_err(CatalogError::MissionTextAllocation)?;
    states.resize(graph.len(), State::Unseen);
    let mut stack = Vec::new();
    begin(&mut states, &mut stack, 0)?;
    while let Some(frame) = stack.last_mut() {
        check_cancel(cancel)?;
        let node = graph.node(frame.cursor.node)?;
        let Some(segment) = node.segments.get(frame.cursor.next) else {
            let id = frame.cursor.node;
            let done = finish(node, frame, &states)?;
            *slot(&mut states, id)? = done;
            stack.pop();
            continue;
        };
        if let Segment::Follow { target, .. } = *segment
            && matches!(state(&states, target)?, State::Unseen)
        {
            begin(&mut states, &mut stack, target)?;
            continue;
        }
        frame.cursor.next += 1;
        frame.bytes = add(frame.bytes, segment, &node.text, &states)?;
    }
    Ok(states)
}

fn begin(states: &mut [State], stack: &mut Vec<Frame>, node: usize) -> Result<(), CatalogError> {
    *slot(states, node)? = State::Active;
    push(stack, Frame::at(node))
}

fn add(
    bytes: Option<usize>,
    segment: &Segment<'_>,
    text: &str,
    states: &[State],
) -> Result<Option<usize>, CatalogError> {
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let size = match *segment {
        Segment::Literal(span) => span.read(text)?.len(),
        Segment::Unresolved(span) => brackets(span.read(text)?)?,
        Segment::Runtime(label) => brackets(label)?,
        Segment::Follow { target, .. } => match state(states, target)? {
            State::Finite { bytes, .. } => bytes,
            State::Active | State::Cyclic => return Ok(None),
            State::Unseen => return Err(CatalogError::MissionTextPlan),
        },
    };
    add_bytes(bytes, size).map(Some)
}

fn finish(node: &Node<'_>, frame: &Frame, states: &[State]) -> Result<State, CatalogError> {
    let Some(bytes) = frame.bytes else {
        return Ok(State::Cyclic);
    };
    let terminal = if let [Segment::Follow { target, .. }] = node.segments.as_slice() {
        terminal(states, *target)?
    } else {
        frame.cursor.node
    };
    Ok(State::Finite { bytes, terminal })
}

pub(super) fn terminal(states: &[State], id: usize) -> Result<usize, CatalogError> {
    match state(states, id)? {
        State::Finite { terminal, .. } => Ok(terminal),
        State::Cyclic => Ok(id),
        State::Active | State::Unseen => Err(CatalogError::MissionTextPlan),
    }
}

pub(super) fn state(states: &[State], id: usize) -> Result<State, CatalogError> {
    states.get(id).copied().ok_or(CatalogError::MissionTextPlan)
}

fn slot(states: &mut [State], id: usize) -> Result<&mut State, CatalogError> {
    states.get_mut(id).ok_or(CatalogError::MissionTextPlan)
}

pub(super) fn brackets(name: &str) -> Result<usize, CatalogError> {
    add_bytes(name.len(), 2)
}

pub(super) fn add_bytes(left: usize, right: usize) -> Result<usize, CatalogError> {
    left.checked_add(right)
        .filter(|bytes| *bytes <= isize::MAX.unsigned_abs())
        .ok_or(CatalogError::MissionTextLength)
}
