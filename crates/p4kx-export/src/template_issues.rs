// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::sync::Mutex;

use thiserror::Error;

/// A contract template whose structure the catalog does not follow to its end.
#[derive(Debug, Error)]
pub(crate) enum TemplateError {
    /// Child mission phases nest deeper than the limit.
    #[error("objective phases nest deeper than {0} levels")]
    PhasesTooDeep(usize),
    /// Mission flow conditions nest deeper than the limit.
    #[error("mission flow conditions nest deeper than {0} levels")]
    ConditionsTooDeep(usize),
}

/// The stored enum names of the contract templates that the catalog does not know, counted per
/// kind of name and value; each one is `null` in the catalog and a warning of the build.
#[derive(Debug, Default)]
pub(crate) struct Unknowns(Mutex<BTreeMap<String, usize>>);

impl Unknowns {
    /// Returns what `parse` makes of the stored name `stored`; a name it does not know is counted
    /// under `what`.
    pub(crate) fn parse<T>(
        &self,
        what: &str,
        stored: Option<&str>,
        parse: impl FnOnce(&str) -> Option<T>,
    ) -> Option<T> {
        let stored = stored?;
        let parsed = parse(stored);
        if parsed.is_none()
            && let Ok(mut counts) = self.0.lock()
        {
            let count = counts.entry(format!("{what} {stored}")).or_insert(0);
            *count = count.saturating_add(1);
        }
        parsed
    }

    /// Returns the number of names counted and one description per kind and value, ordered.
    pub(crate) fn into_details(self) -> (usize, Vec<String>) {
        let counts = self.0.into_inner().unwrap_or_default();
        let total = counts
            .values()
            .copied()
            .fold(0_usize, usize::saturating_add);
        let details = counts
            .into_iter()
            .map(|(name, count)| format!("{name} ({count})"))
            .collect();
        (total, details)
    }
}
