// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::ptr;

use p4kx_dataforge::{DataForge, Record, RecordTree, Value};
use p4kx_extraction::{RecordJsonLocation, SourceLocation};

use crate::LoadoutSelection;
use crate::read;
use crate::resolver_observation::Observation;

/// An original DataForge value accepted by the XML Loadout path getter.
pub struct LoadoutXmlRecordSelector<'site, 'source> {
    record: &'site Record<'source>,
    selection: &'site LoadoutSelection,
    ordinal: usize,
    value: &'site Value<'source>,
}

impl<'site, 'source> LoadoutXmlRecordSelector<'site, 'source> {
    /// Borrows the physical Record whose projection read the selector.
    #[must_use]
    pub const fn record(&self) -> &'site Record<'source> {
        self.record
    }

    /// Borrows the exact path token minted by this actual getter.
    #[must_use]
    pub const fn selection(&self) -> &'site LoadoutSelection {
        self.selection
    }

    /// Returns the original value's preorder ordinal in that Record.
    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }

    /// Addresses the original string bytes within the actual owning DataForge.
    #[must_use]
    pub fn text_location(&self, forge: &DataForge<'_>) -> Option<SourceLocation> {
        if !self.record.is_from(forge) {
            return None;
        }
        RecordJsonLocation::borrowed_text(forge, read::value_text(self.value))
    }
}

#[derive(Default)]
pub(crate) struct XmlRecordPaths {
    pending: RefCell<HashMap<usize, Vec<LoadoutSelection>>>,
}

impl XmlRecordPaths {
    pub(crate) fn capture(
        &self,
        value: &Value<'_>,
        selection: &LoadoutSelection,
    ) -> ControlFlow<()> {
        let mut pending = self.pending.borrow_mut();
        let address = ptr::from_ref(value).addr();
        if !pending.contains_key(&address) && pending.try_reserve(1).is_err() {
            return ControlFlow::Break(());
        }
        let selections = pending.entry(address).or_default();
        if selections.try_reserve(1).is_err() {
            return ControlFlow::Break(());
        }
        selections.push(selection.clone());
        ControlFlow::Continue(())
    }

    pub(crate) fn finish<'source>(
        self,
        record: &Record<'source>,
        tree: &RecordTree<'source>,
        observation: &Observation<'_>,
    ) -> ControlFlow<()> {
        let mut pending = self.pending.into_inner();
        if pending.is_empty() {
            return ControlFlow::Continue(());
        }
        let mut outcome = ControlFlow::Continue(());
        tree.root().visit_owned_values(|visit| {
            if outcome.is_break() || pending.is_empty() {
                return;
            }
            let Some(selections) = pending.remove(&ptr::from_ref(visit.value()).addr()) else {
                return;
            };
            for selection in &selections {
                let site = LoadoutXmlRecordSelector {
                    record,
                    selection,
                    ordinal: visit.ordinal(),
                    value: visit.value(),
                };
                outcome = observation.xml_record_selector(&site);
                if outcome.is_break() {
                    break;
                }
            }
        });
        if outcome.is_break() || !pending.is_empty() {
            observation.stop();
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    }
}
