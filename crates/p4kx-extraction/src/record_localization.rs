// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{LocalizationInput, ObservedResolution, Resolution, TextResolver};
use p4kx_localization::{GlobalIni, is_placeholder_key};

use super::display;
use crate::localization::LocalizationValueEvidence;
use crate::{
    InputScope, LocalizationEntry, LocalizationLoadError, LocalizationNote, SourceEvidenceError,
    SourceInput, SourceUse, SourceUseRole, SourceUses,
};

/// A language Entry indexed together with the original Record annotation input.
pub struct RecordLocalization<'entry> {
    entry: &'entry LocalizationEntry,
    ini: GlobalIni<'entry>,
}

impl<'entry> RecordLocalization<'entry> {
    /// Indexes the exact loaded language Entry for Record annotations.
    ///
    /// # Errors
    /// Returns a localization indexing error.
    pub fn try_new(entry: &'entry LocalizationEntry) -> Result<Self, LocalizationLoadError> {
        Ok(Self {
            entry,
            ini: entry.index()?,
        })
    }

    /// Returns the diagnostic for undecodable bytes or skipped lines.
    #[must_use]
    pub fn note(&self) -> Option<LocalizationNote> {
        LocalizationNote::for_entry(self.entry, self.ini.skipped())
    }

    pub(crate) fn capture_lookup(
        &self,
        observation: &ObservedResolution<'_>,
    ) -> Option<LocalizationLookupEvidence<'entry>> {
        let LocalizationInput::Lookup(original) = observation.input() else {
            return None;
        };
        Some(LocalizationLookupEvidence::new(
            self.entry,
            original,
            !self.entry.text().is_lossy() && self.ini.skipped() == 0,
        ))
    }
}

pub(crate) struct LocalizationLookupEvidence<'entry> {
    entry: &'entry LocalizationEntry,
    value: Option<LocalizationValueEvidence<'entry>>,
    complete: bool,
}

impl<'entry> LocalizationLookupEvidence<'entry> {
    pub(super) fn new(
        entry: &'entry LocalizationEntry,
        original: Option<&str>,
        complete_absence: bool,
    ) -> Self {
        let value = original.and_then(|value| entry.value_evidence(value));
        let complete = original.map_or(complete_absence, |_| value.is_some());
        Self {
            entry,
            value,
            complete,
        }
    }

    pub(crate) fn append_uses(
        &self,
        registry: &SourceUses,
        authored_key: Option<&SourceUse>,
        output: &mut Vec<SourceUse>,
    ) -> Result<bool, SourceEvidenceError> {
        let original = self.entry.origin();
        if let Some(key) = authored_key {
            let scope = registry.observe(
                original.map_or(SourceInput::Unknown, SourceInput::Archive),
                InputScope::Payload,
            )?;
            output.push(scope.at_localization_lookup(
                self.entry.language(),
                key,
                SourceUseRole::Conversion,
            )?);
        }
        let value_complete = match &self.value {
            Some(value) => value.append_uses(registry, output)?,
            None => true,
        };
        Ok(self.complete
            && value_complete
            && authored_key.is_some()
            && original.is_some_and(|entry| entry.ancestors().all(|step| step.is_some())))
    }
}

impl TextResolver for RecordLocalization<'_> {
    fn resolve<'resolver>(&'resolver self, key: &str) -> Resolution<'resolver> {
        self.resolve_observed(key).into_resolution()
    }

    fn resolve_observed<'resolver>(&'resolver self, key: &str) -> ObservedResolution<'resolver> {
        if is_placeholder_key(key) {
            return ObservedResolution::unobserved(Resolution::Keep);
        }
        let original = self.ini.resolve(key);
        ObservedResolution::from_lookup(display(original), original)
    }
}
