// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use crate::query::reader::{Column, Reader};
use crate::query::selection_control::Control;
use crate::query::sink::{Reached, Sink};
use crate::query::source::{self, QueryByteUse, QuerySource, TraversalInput};
use crate::query::traversal_gap;
use crate::query::walk::Place;
use crate::{DataForge, Value, ValueError};

/// An existential comparison that keeps only its truth value and first unreadable value.
struct Matching<'s, 'f> {
    forge: &'f DataForge<'f>,
    leaf: &'s Column,
    control: &'s Control<'s>,
    test: &'s dyn Fn(&Value<'f>) -> Option<bool>,
    result: Option<bool>,
    error: Option<ValueError>,
}

impl Matching<'_, '_> {
    fn finish(self) -> Result<Option<bool>, ValueError> {
        if self.result == Some(true) {
            return Ok(Some(true));
        }
        self.error.map_or(Ok(self.result), Err)
    }

    fn observe_value(&self, place: Option<Place<'_>>, bytes: Option<&[u8]>, value: &Value<'_>) {
        let Some(observe) = self.control.observer() else {
            return;
        };
        let details = self.control.predicate_observer().is_some();
        if matches!(value, Value::Error(_)) && details {
            let attempted = bytes
                .and_then(|bytes| {
                    QuerySource::bytes(self.forge, bytes, QueryByteUse::AttemptedValue)
                })
                .or_else(|| {
                    place.map(|place| {
                        traversal_gap::unavailable(
                            self.forge,
                            place,
                            self.leaf.start,
                            self.leaf.end.saturating_sub(self.leaf.start),
                            TraversalInput::Value,
                        )
                    })
                });
            if let Some(source) = attempted {
                observe(source);
            }
            return;
        }
        for source in source::value_sources(self.forge, bytes, value, details)
            .into_iter()
            .flatten()
        {
            observe(source);
        }
    }
}

impl<'f> Sink<'f> for Matching<'_, 'f> {
    fn proceed(&mut self) -> ControlFlow<()> {
        if self.control.proceed() {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }

    fn charge(&mut self, _elements: usize) -> Result<(), ValueError> {
        Ok(())
    }

    fn source_depth(&self) -> Option<usize> {
        self.control.observer().map(|_| 0)
    }

    fn route(&mut self, source: QuerySource) {
        if let Some(observe) = self.control.observer() {
            observe(source);
        }
    }

    fn predicate_observer(&self) -> Option<&(dyn Fn(QuerySource) + Sync)> {
        self.control.predicate_observer()
    }

    fn unresolved(&mut self, _link: &'f [u8]) {}

    fn reached(&mut self, reached: Reached<'f>) -> ControlFlow<()> {
        let bytes = self
            .control
            .observer()
            .and_then(|_| reached.bytes(self.leaf));
        let place = match &reached {
            Reached::Place(place) => Some(*place),
            Reached::Value(_) => None,
        };
        let value = match (self.control.predicate_observer(), reached) {
            (Some(observe), Reached::Place(place)) => self.leaf.read_observed(
                self.forge,
                place,
                Some(&mut |scalar| observe(scalar.into())),
            ),
            (_, reached) => reached.read(self.forge, self.leaf),
        };
        self.observe_value(place, bytes, &value);
        if let Value::Error(error) = value {
            self.error.get_or_insert(error);
            return ControlFlow::Continue(());
        }
        match (self.test)(&value) {
            Some(true) => {
                self.result = Some(true);
                ControlFlow::Break(())
            }
            Some(false) => ControlFlow::Continue(()),
            None => {
                self.result = None;
                ControlFlow::Continue(())
            }
        }
    }
}

impl Reader {
    /// Tests values as they are reached, stopping at a witness or when selection stops.
    pub(super) fn any<'f>(
        &self,
        forge: &'f DataForge<'f>,
        place: Place<'f>,
        control: &Control<'_>,
        test: impl Fn(&Value<'f>) -> Option<bool>,
    ) -> Result<Option<bool>, ValueError> {
        let leaf = match self {
            Self::Column(column) => column,
            Self::Walk(walk) => &walk.leaf,
        };
        let mut sink = Matching {
            forge,
            leaf,
            control,
            test: &test,
            result: Some(false),
            error: None,
        };
        match self {
            Self::Column(_) if sink.proceed().is_continue() => {
                let _flow = sink.reached(Reached::Place(place));
            }
            Self::Column(_) => {}
            Self::Walk(walk) => {
                let _flow = walk.step(forge, 0, walk.steps.len(), place, &mut sink)?;
            }
        }
        sink.finish()
    }
}
