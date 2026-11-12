// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::query::QuerySource;

use crate::budget::MAX_RESPONSE_LEN;

#[derive(Debug)]
pub(crate) struct SelectionReceipt {
    pub(crate) records: Box<[u32]>,
    pub(crate) survey: Survey,
}

#[derive(Clone, Copy)]
pub(crate) enum SurveyMode {
    Complete,
    Witness,
}

#[derive(Debug, Default)]
pub(crate) struct Survey {
    sources: BTreeSet<QuerySource>,
    omitted: bool,
}

impl Survey {
    pub(crate) fn record(&mut self, source: QuerySource) {
        const MINIMUM_USE_BYTES: usize = br#"{"id":"","location":{}}"#.len();
        if self.sources.insert(source)
            && self.sources.len().saturating_mul(MINIMUM_USE_BYTES) > MAX_RESPONSE_LEN
        {
            self.sources.pop_last();
            self.omitted = true;
        }
    }

    pub(crate) fn selected(&self, mode: SurveyMode) -> impl Iterator<Item = QuerySource> + '_ {
        let sources = match mode {
            SurveyMode::Complete => self.sources.len(),
            SurveyMode::Witness => 1,
        };
        self.sources.iter().copied().take(sources)
    }

    pub(crate) fn complete(&self, mode: SurveyMode) -> bool {
        !self.omitted && (matches!(mode, SurveyMode::Complete) || self.sources.len() <= 1)
    }
}
