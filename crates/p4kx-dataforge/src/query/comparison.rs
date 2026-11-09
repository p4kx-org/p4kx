// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use crate::query::reader::{Column, Reader};
use crate::query::sink::{Reached, Sink};
use crate::query::walk::Place;
use crate::{DataForge, Value, ValueError};

/// An existential comparison that keeps only its truth value and first unreadable value.
struct Matching<'s, 'f> {
    forge: &'f DataForge<'f>,
    leaf: &'s Column,
    proceed: &'s dyn Fn() -> bool,
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
}

impl<'f> Sink<'f> for Matching<'_, 'f> {
    fn proceed(&mut self) -> ControlFlow<()> {
        if (self.proceed)() {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    }

    fn charge(&mut self, _elements: usize) -> Result<(), ValueError> {
        Ok(())
    }

    fn unresolved(&mut self, _link: &[u8]) {}

    fn reached(&mut self, reached: Reached<'f>) -> ControlFlow<()> {
        let value = reached.read(self.forge, self.leaf);
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
    /// Tests values as they are reached, stopping at a witness or when `proceed` declines work.
    pub(super) fn any<'f>(
        &self,
        forge: &'f DataForge<'f>,
        place: Place<'f>,
        proceed: &dyn Fn() -> bool,
        test: impl Fn(&Value<'f>) -> Option<bool>,
    ) -> Result<Option<bool>, ValueError> {
        let leaf = match self {
            Self::Column(column) => column,
            Self::Walk(walk) => &walk.leaf,
        };
        let mut sink = Matching {
            forge,
            leaf,
            proceed,
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
