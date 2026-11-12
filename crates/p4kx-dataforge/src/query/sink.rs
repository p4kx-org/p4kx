// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use crate::query::MAX_PATH_VALUES;
use crate::query::reader::{Column, Step, Walk};
use crate::query::source::{CellTrace, QueryByteUse, QuerySource};
use crate::query::state::WalkState;
use crate::query::walk::Place;
use crate::{DataForge, Value, ValueError};

/// What a step reaches: the place the path continues at, or a value that ends it.
#[derive(Debug)]
pub(crate) enum Reached<'f> {
    Place(Place<'f>),
    Value(Value<'f>),
}

impl<'f> Reached<'f> {
    pub(super) fn bytes(&self, leaf: &Column) -> Option<&'f [u8]> {
        match self {
            Self::Place(place) => leaf.bytes(*place),
            Self::Value(_) => None,
        }
    }

    /// Reads the reached place or returns its terminal value.
    pub(super) fn read(self, forge: &'f DataForge<'f>, leaf: &Column) -> Value<'f> {
        match self {
            Self::Place(place) => leaf.read(forge, place),
            Self::Value(value) => value,
        }
    }
}

/// Where a walk sends what it charges, counts and reaches.
pub(crate) trait Sink<'f> {
    /// Returns whether the walk may continue at its next step or array element.
    fn proceed(&mut self) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Counts `elements` more array elements.
    fn charge(&mut self, elements: usize) -> Result<(), ValueError>;

    fn source_depth(&self) -> Option<usize> {
        None
    }

    fn restore_sources(&mut self, _depth: usize) {}

    fn route(&mut self, _source: QuerySource) {}

    fn follow_decision(&mut self, _step: usize) {}

    fn route_bytes(&mut self, forge: &DataForge<'_>, bytes: &[u8], kind: QueryByteUse) {
        if self.source_depth().is_some()
            && let Some(source) = QuerySource::bytes(forge, bytes, kind)
        {
            self.route(source);
        }
    }

    /// Counts a link that the path cannot follow.
    fn unresolved(&mut self, link: &'f [u8]);

    /// Takes what the walk reached; `Break` ends the walk.
    fn reached(&mut self, reached: Reached<'f>) -> ControlFlow<()>;
}

/// The walk of one path: values are read at the leaf and given to `emit`.
pub(crate) struct Live<'s, 'f> {
    pub(crate) forge: &'f DataForge<'f>,
    pub(crate) leaf: &'s Column,
    pub(crate) state: &'s mut WalkState,
    pub(crate) emit: &'s mut dyn FnMut(Value<'f>) -> ControlFlow<()>,
    pub(super) trace: Option<&'s mut CellTrace>,
    pub(super) last_follow: Option<usize>,
}

impl<'f> Sink<'f> for Live<'_, 'f> {
    fn charge(&mut self, elements: usize) -> Result<(), ValueError> {
        self.state.charge(elements)
    }

    fn source_depth(&self) -> Option<usize> {
        self.trace.as_ref().map(|trace| trace.depth())
    }

    fn restore_sources(&mut self, depth: usize) {
        if let Some(trace) = &mut self.trace {
            trace.restore(depth);
        }
    }

    fn route(&mut self, source: QuerySource) {
        if let Some(trace) = &mut self.trace {
            trace.route(source);
        }
    }

    fn follow_decision(&mut self, step: usize) {
        if self.last_follow.is_some_and(|last| step <= last)
            && let Some(trace) = &mut self.trace
        {
            trace.observe_follow_route();
        }
    }

    fn unresolved(&mut self, link: &'f [u8]) {
        self.state.note_unresolved(link.as_ptr().addr());
        if let Some(trace) = &mut self.trace
            && let Some(source) = QuerySource::bytes(self.forge, link, QueryByteUse::Unresolved)
        {
            trace.unresolved(source);
        }
    }

    fn reached(&mut self, reached: Reached<'f>) -> ControlFlow<()> {
        let bytes = self.trace.as_ref().and_then(|_| reached.bytes(self.leaf));
        let value = reached.read(self.forge, self.leaf);
        if let Some(trace) = &mut self.trace {
            trace.value(self.forge, bytes, &value);
        }
        (self.emit)(value)
    }
}

/// One thing a shared prefix did, in the order it did it.
#[derive(Debug)]
pub(crate) enum Event<'f> {
    Charge(usize),
    Unresolved(&'f [u8]),
    Route(QuerySource),
    Restore(usize),
    FollowDecision(usize),
    Reached(Reached<'f>),
}

/// The walk of the steps several paths share, kept to be replayed once per path.
#[derive(Debug, Default)]
pub(crate) struct Recording<'f> {
    events: Vec<Event<'f>>,
    charged: usize,
    source_depth: Option<usize>,
    failed: bool,
    filled: bool,
}

impl<'f> Recording<'f> {
    /// Forgets the walk but keeps the buffer for the next one.
    pub(crate) fn clear(&mut self) {
        self.events.clear();
        self.charged = 0;
        self.failed = false;
        self.filled = false;
    }

    pub(super) fn observe_sources(&mut self, enabled: bool) {
        self.source_depth = enabled.then_some(0);
    }

    /// Returns whether a walk is recorded since the last [`clear`](Self::clear).
    pub(crate) const fn is_filled(&self) -> bool {
        self.filled
    }

    /// Walks the first `prefix` steps of `walk` from `place` and records what they do, replacing
    /// the earlier recording.
    pub(crate) fn fill(
        &mut self,
        walk: &Walk,
        forge: &'f DataForge<'f>,
        place: Place<'f>,
        prefix: usize,
    ) {
        self.clear();
        self.filled = true;
        if walk.step(forge, 0, prefix, place, self).is_err() {
            self.failed = true;
        }
    }
}

impl<'f> Sink<'f> for Recording<'f> {
    fn charge(&mut self, elements: usize) -> Result<(), ValueError> {
        let charged = self
            .charged
            .checked_add(elements)
            .filter(|charged| *charged <= MAX_PATH_VALUES)
            .ok_or(ValueError::PathTooWide {
                limit: MAX_PATH_VALUES,
            })?;
        self.charged = charged;
        self.events.push(Event::Charge(elements));
        Ok(())
    }

    fn source_depth(&self) -> Option<usize> {
        self.source_depth
    }

    fn restore_sources(&mut self, depth: usize) {
        if let Some(active) = &mut self.source_depth
            && *active != depth
        {
            *active = depth;
            self.events.push(Event::Restore(depth));
        }
    }

    fn route(&mut self, source: QuerySource) {
        if let Some(depth) = &mut self.source_depth {
            *depth += 1;
            self.events.push(Event::Route(source));
        }
    }

    fn follow_decision(&mut self, step: usize) {
        if self.source_depth.is_some() {
            self.events.push(Event::FollowDecision(step));
        }
    }

    fn unresolved(&mut self, link: &'f [u8]) {
        self.events.push(Event::Unresolved(link));
    }

    fn reached(&mut self, reached: Reached<'f>) -> ControlFlow<()> {
        self.events.push(Event::Reached(reached));
        ControlFlow::Continue(())
    }
}

impl Walk {
    pub(super) fn last_follow(&self) -> Option<usize> {
        self.steps
            .iter()
            .rposition(|step| matches!(step, Step::Follow { .. }))
    }

    /// Calls `emit` with each value the path reaches in order, as [`Walk::visit`] does, taking
    /// the steps before `prefix` from `recording` instead of walking them.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::PathTooWide`] under the conditions of [`Walk::visit`].
    pub(super) fn replay<'f>(
        &self,
        forge: &'f DataForge<'f>,
        recording: &Recording<'f>,
        prefix: usize,
        state: &mut WalkState,
        emit: &mut dyn FnMut(Value<'f>) -> ControlFlow<()>,
        trace: Option<&mut CellTrace>,
    ) -> Result<(), ValueError> {
        let last_follow = trace.as_ref().and_then(|_| self.last_follow());
        let mut live = Live {
            forge,
            leaf: &self.leaf,
            state,
            emit,
            trace,
            last_follow,
        };
        for event in &recording.events {
            let flow = match event {
                Event::Charge(elements) => {
                    live.charge(*elements)?;
                    continue;
                }
                Event::Unresolved(bytes) => {
                    live.unresolved(bytes);
                    continue;
                }
                Event::Route(source) => {
                    live.route(*source);
                    continue;
                }
                Event::Restore(depth) => {
                    live.restore_sources(*depth);
                    continue;
                }
                Event::FollowDecision(step) => {
                    live.follow_decision(*step);
                    continue;
                }
                Event::Reached(Reached::Place(place)) => {
                    self.step(forge, prefix, self.steps.len(), *place, &mut live)?
                }
                Event::Reached(Reached::Value(value)) => {
                    live.reached(Reached::Value(value.clone()))
                }
            };
            if flow.is_break() {
                return Ok(());
            }
        }
        if recording.failed {
            return Err(ValueError::PathTooWide {
                limit: MAX_PATH_VALUES,
            });
        }
        Ok(())
    }
}
