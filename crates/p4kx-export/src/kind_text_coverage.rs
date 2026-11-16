// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::collections::HashMap;

use p4kx_extraction::Language;

use crate::Kind;
use crate::texts::{CATALOG_LANGUAGES, TextProjection, Texts};

pub(super) type Translations<'source> = BTreeMap<Language, Option<TextProjection<'source>>>;

/// A catalog dataset that registers authored translation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextOwner {
    /// An existing catalog Kind with canonical text registrations.
    Catalog(Kind),
    /// Localized notifications of the recovery policy.
    ItemRecoveryPolicy,
    /// Localized subtype names and effect descriptions of consumables.
    ConsumableSubtypes,
    /// Localized display names of physical item port definitions.
    ItemPorts,
}

impl TextOwner {
    /// Returns the stable diagnostic identifier of the dataset.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Catalog(kind) => kind.name(),
            Self::ItemRecoveryPolicy => "configurations.itemRecoveryPolicy",
            Self::ConsumableSubtypes => "definitions.consumableSubtypes",
            Self::ItemPorts => "configurations.itemPorts",
        }
    }
}

/// Canonical authored lookup registrations for one catalog dataset and language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct KindTextSummary {
    /// The catalog dataset that registered the text.
    pub owner: TextOwner,
    /// The requested language.
    pub language: Language,
    /// The requested language has a matching key and a nonempty display form.
    pub resolved: usize,
    /// No source was loaded for the requested language.
    pub missing_language: usize,
    /// The requested language exists, but its index has no matching key.
    pub missing_key: usize,
    /// The winning original value is an empty string.
    pub empty_source: usize,
    /// The winning original value becomes empty after display conversion.
    pub empty_display: usize,
    /// The registered token is a recognized placeholder and needs no lookup.
    pub placeholder: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Resolved,
    MissingLanguage,
    MissingKey,
    EmptySource,
    EmptyDisplay,
    Placeholder,
}

#[derive(Clone, Copy)]
pub(super) enum Lookup<'lookup, 'source> {
    Placeholder,
    Projected(&'lookup Translations<'source>),
}

impl<'lookup, 'source> Lookup<'lookup, 'source> {
    pub(super) const fn from_projection(
        translations: &'lookup Translations<'source>,
        looked_up: bool,
    ) -> Self {
        if looked_up {
            Self::Projected(translations)
        } else {
            Self::Placeholder
        }
    }

    pub(super) fn recount(self, texts: &Texts<'_>) {
        match self {
            Self::Placeholder => texts.note_placeholder(),
            Self::Projected(_) => {
                texts.note_languages(|language| self.outcome(language) == Outcome::Resolved);
            }
        }
    }

    fn outcome(self, language: Language) -> Outcome {
        match self {
            Self::Placeholder => Outcome::Placeholder,
            Self::Projected(translations) => match translations.get(&language) {
                None => Outcome::MissingLanguage,
                Some(None) => Outcome::MissingKey,
                Some(Some(text)) if text.source.is_empty() => Outcome::EmptySource,
                Some(Some(text)) if text.display.is_empty() => Outcome::EmptyDisplay,
                Some(Some(_)) => Outcome::Resolved,
            },
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Totals {
    resolved: usize,
    missing_language: usize,
    missing_key: usize,
    empty_source: usize,
    empty_display: usize,
    placeholder: usize,
}

impl Totals {
    const fn note(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Resolved => self.resolved += 1,
            Outcome::MissingLanguage => self.missing_language += 1,
            Outcome::MissingKey => self.missing_key += 1,
            Outcome::EmptySource => self.empty_source += 1,
            Outcome::EmptyDisplay => self.empty_display += 1,
            Outcome::Placeholder => self.placeholder += 1,
        }
    }

    const fn summary(self, owner: TextOwner, language: Language) -> KindTextSummary {
        KindTextSummary {
            owner,
            language,
            resolved: self.resolved,
            missing_language: self.missing_language,
            missing_key: self.missing_key,
            empty_source: self.empty_source,
            empty_display: self.empty_display,
            placeholder: self.placeholder,
        }
    }
}

#[derive(Default)]
pub(super) struct KindTextCoverage {
    by_owner: HashMap<TextOwner, [Totals; CATALOG_LANGUAGES.len()]>,
}

impl KindTextCoverage {
    pub(super) fn record(&mut self, owner: TextOwner, lookup: Lookup<'_, '_>) {
        let counts = self.by_owner.entry(owner).or_default();
        for (total, language) in counts.iter_mut().zip(CATALOG_LANGUAGES) {
            total.note(lookup.outcome(language));
        }
    }

    pub(super) fn finish(mut self) -> Vec<KindTextSummary> {
        let mut summaries = Vec::new();
        let owners = Kind::ALL.into_iter().map(TextOwner::Catalog).chain([
            TextOwner::ItemRecoveryPolicy,
            TextOwner::ConsumableSubtypes,
            TextOwner::ItemPorts,
        ]);
        for owner in owners {
            if let Some(counts) = self.by_owner.remove(&owner) {
                summaries.extend(
                    counts
                        .into_iter()
                        .zip(CATALOG_LANGUAGES)
                        .map(|(total, language)| total.summary(owner, language)),
                );
            }
        }
        summaries
    }
}

#[cfg(test)]
#[path = "kind_text_coverage_tests.rs"]
mod tests;
