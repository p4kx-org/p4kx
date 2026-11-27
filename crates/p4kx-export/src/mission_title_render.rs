// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;

use super::graph::{Graph, Segment};
use super::plan::{Cursor, State, add_bytes, brackets, state, terminal};
use super::{check_cancel, push};
use crate::CatalogError;

pub(super) fn write(
    graph: &Graph<'_>,
    states: &[State],
    cancel: Option<&CancelToken>,
) -> Result<String, CatalogError> {
    let mut output = String::new();
    let cyclic = match state(states, 0)? {
        State::Finite { bytes, .. } => {
            output
                .try_reserve_exact(bytes)
                .map_err(CatalogError::MissionTextAllocation)?;
            false
        }
        State::Cyclic => true,
        State::Active | State::Unseen => return Err(CatalogError::MissionTextPlan),
    };
    let mut writer = Writer {
        output,
        active: Vec::new(),
        stack: Vec::new(),
        states,
    };
    if cyclic {
        writer
            .active
            .try_reserve_exact(graph.len())
            .map_err(CatalogError::MissionTextAllocation)?;
        writer.active.resize(graph.len(), false);
    }
    writer.enter(terminal(states, 0)?)?;
    writer.write(graph, cancel)?;
    Ok(writer.output)
}

struct Writer<'plan> {
    output: String,
    active: Vec<bool>,
    stack: Vec<Cursor>,
    states: &'plan [State],
}

impl Writer<'_> {
    fn write(
        &mut self,
        graph: &Graph<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        while let Some(cursor) = self.stack.last_mut() {
            check_cancel(cancel)?;
            let node = graph.node(cursor.node)?;
            let Some(segment) = node.segments.get(cursor.next).copied() else {
                self.leave()?;
                continue;
            };
            cursor.next += 1;
            match segment {
                Segment::Literal(span) => self.literal(span.read(&node.text)?)?,
                Segment::Unresolved(span) => self.bracket(span.read(&node.text)?, true)?,
                Segment::Runtime(label) => self.bracket(label, false)?,
                Segment::Follow { target, name } => {
                    let target = terminal(self.states, target)?;
                    if self.active.get(target).copied() == Some(true) {
                        self.bracket(name.read(&node.text)?, true)?;
                    } else {
                        self.enter(target)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn enter(&mut self, node: usize) -> Result<(), CatalogError> {
        self.mark(node, true)?;
        push(&mut self.stack, Cursor::at(node))
    }

    fn leave(&mut self) -> Result<(), CatalogError> {
        let cursor = self.stack.pop().ok_or(CatalogError::MissionTextPlan)?;
        self.mark(cursor.node, false)
    }

    fn mark(&mut self, node: usize, active: bool) -> Result<(), CatalogError> {
        if !self.active.is_empty() {
            *self
                .active
                .get_mut(node)
                .ok_or(CatalogError::MissionTextPlan)? = active;
        }
        Ok(())
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), CatalogError> {
        add_bytes(self.output.len(), bytes)?;
        self.output
            .try_reserve(bytes)
            .map_err(CatalogError::MissionTextAllocation)
    }

    fn literal(&mut self, text: &str) -> Result<(), CatalogError> {
        self.reserve(text.len())?;
        self.output.push_str(text);
        Ok(())
    }

    fn bracket(&mut self, name: &str, uppercase: bool) -> Result<(), CatalogError> {
        self.reserve(brackets(name)?)?;
        self.output.push('[');
        if uppercase {
            self.output
                .extend(name.chars().map(|character| character.to_ascii_uppercase()));
        } else {
            self.output.push_str(name);
        }
        self.output.push(']');
        Ok(())
    }
}
