// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use crate::query::MAX_PATH_VALUES;
use crate::query::reader::{Column, Walk};
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

    /// Counts the link stored in `link`, which `->` cannot follow.
    fn unresolved(&mut self, link: &[u8]);

    /// Takes what the walk reached; `Break` ends the walk.
    fn reached(&mut self, reached: Reached<'f>) -> ControlFlow<()>;
}

/// The walk of one path: values are read at the leaf and given to `emit`.
pub(crate) struct Live<'s, 'f> {
    pub(crate) forge: &'f DataForge<'f>,
    pub(crate) leaf: &'s Column,
    pub(crate) state: &'s mut WalkState,
    pub(crate) emit: &'s mut dyn FnMut(Value<'f>) -> ControlFlow<()>,
}

impl<'f> Sink<'f> for Live<'_, 'f> {
    fn charge(&mut self, elements: usize) -> Result<(), ValueError> {
        self.state.charge(elements)
    }

    fn unresolved(&mut self, link: &[u8]) {
        self.state.note_unresolved(link.as_ptr().addr());
    }

    fn reached(&mut self, reached: Reached<'f>) -> ControlFlow<()> {
        let value = reached.read(self.forge, self.leaf);
        (self.emit)(value)
    }
}

/// One thing a shared prefix did, in the order it did it.
#[derive(Debug)]
pub(crate) enum Event<'f> {
    Charge(usize),
    Unresolved(usize),
    Reached(Reached<'f>),
}

/// The walk of the steps several paths share, kept to be replayed once per path.
#[derive(Debug, Default)]
pub(crate) struct Recording<'f> {
    events: Vec<Event<'f>>,
    charged: usize,
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

    fn unresolved(&mut self, link: &[u8]) {
        self.events.push(Event::Unresolved(link.as_ptr().addr()));
    }

    fn reached(&mut self, reached: Reached<'f>) -> ControlFlow<()> {
        self.events.push(Event::Reached(reached));
        ControlFlow::Continue(())
    }
}

impl Walk {
    /// Calls `emit` with each value the path reaches in order, as [`Walk::visit`] does, taking
    /// the steps before `prefix` from `recording` instead of walking them.
    ///
    /// # Errors
    ///
    /// Returns [`ValueError::PathTooWide`] under the conditions of [`Walk::visit`].
    pub(crate) fn replay<'f>(
        &self,
        forge: &'f DataForge<'f>,
        recording: &Recording<'f>,
        prefix: usize,
        state: &mut WalkState,
        emit: &mut dyn FnMut(Value<'f>) -> ControlFlow<()>,
    ) -> Result<(), ValueError> {
        let mut live = Live {
            forge,
            leaf: &self.leaf,
            state,
            emit,
        };
        for event in &recording.events {
            let flow = match event {
                Event::Charge(elements) => {
                    live.charge(*elements)?;
                    continue;
                }
                Event::Unresolved(at) => {
                    live.state.note_unresolved(*at);
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
