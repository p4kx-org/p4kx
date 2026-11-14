// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(crate) use p4kx_extraction::sound::wwise_path::{
    LooseName, SOUND_FOLDER, bank_language, loose_name,
};

/// The language the audio of an Archive is read in: the one asked for and the one used when the
/// first has no localisation of a sound.
///
/// Language names are the folder names the Archive holds below `Data\Sounds\wwise\`, e.g.
/// `English(US)`, compared ignoring ASCII case. A choice without a name is *any*: no language is
/// preferred, every localised soundbank is followed and a loose localised WEM is taken from the
/// first language folder in path order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LanguageChoice {
    preferred: Option<Box<str>>,
    fallback: Option<Box<str>>,
}

impl LanguageChoice {
    /// Returns the choice that prefers no language.
    #[must_use]
    pub const fn any() -> Self {
        Self {
            preferred: None,
            fallback: None,
        }
    }

    /// Returns the choice of the language `preferred`, falling back to `fallback` where a sound
    /// has no localisation in the first; either may be `None`.
    #[must_use]
    pub fn new(preferred: Option<&str>, fallback: Option<&str>) -> Self {
        Self {
            preferred: preferred.map(Into::into),
            fallback: fallback.map(Into::into),
        }
    }

    /// Returns the language asked for.
    #[must_use]
    pub fn preferred(&self) -> Option<&str> {
        self.preferred.as_deref()
    }

    /// Returns the language used where the preferred one has no localisation.
    #[must_use]
    pub fn fallback(&self) -> Option<&str> {
        self.fallback.as_deref()
    }

    /// Returns whether the choice names no language at all.
    #[must_use]
    pub const fn is_any(&self) -> bool {
        self.preferred.is_none() && self.fallback.is_none()
    }

    pub(crate) fn candidates(&self) -> impl Iterator<Item = &str> {
        self.preferred().into_iter().chain(self.fallback())
    }

    pub(crate) fn no_localisation(&self) -> super::BreakReason {
        super::BreakReason::NoLocalisation {
            language: self.preferred().unwrap_or_default().into(),
            fallback: self.fallback().unwrap_or_default().into(),
        }
    }

    pub(crate) fn is_fallback(&self, language: Option<&str>) -> bool {
        match (self.preferred(), language) {
            (Some(preferred), Some(language)) => !preferred.eq_ignore_ascii_case(language),
            _ => false,
        }
    }
}
