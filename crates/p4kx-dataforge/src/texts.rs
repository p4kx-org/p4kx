// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use crate::Annotations;

/// How a [`TextResolver`] answers for one localisation key.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Resolution<'a> {
    /// The key has this text.
    Text(Cow<'a, str>),
    /// The key has no text in the chosen language; the output marks it.
    Missing,
    /// The key stands for no text, e.g. a placeholder key; the output keeps the key alone.
    Keep,
}

/// The original input consulted by a Record text resolver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LocalizationInput<'a> {
    /// The resolver did not retain its lookup.
    Unobserved,
    /// The winning value of a lookup, including an empty value or an absent key.
    Lookup(Option<&'a str>),
}

/// A Record annotation decision and the input consulted to make it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedResolution<'a> {
    resolution: Resolution<'a>,
    input: LocalizationInput<'a>,
}

impl<'a> ObservedResolution<'a> {
    /// Wraps a resolution without its original lookup.
    #[must_use]
    pub const fn unobserved(resolution: Resolution<'a>) -> Self {
        Self {
            resolution,
            input: LocalizationInput::Unobserved,
        }
    }

    /// Wraps a resolution and the winning input of its actual lookup.
    #[must_use]
    pub const fn from_lookup(resolution: Resolution<'a>, input: Option<&'a str>) -> Self {
        Self {
            resolution,
            input: LocalizationInput::Lookup(input),
        }
    }

    /// Returns the value decision to render.
    #[must_use]
    pub const fn resolution(&self) -> &Resolution<'a> {
        &self.resolution
    }

    /// Returns the lookup input retained by the resolver.
    #[must_use]
    pub const fn input(&self) -> LocalizationInput<'a> {
        self.input
    }

    /// Returns the value decision without its input.
    #[must_use]
    pub fn into_resolution(self) -> Resolution<'a> {
        self.resolution
    }
}

/// Resolves the localisation keys of Record fields into the texts of one chosen language, for the
/// writers of Record XML and Record JSON.
pub trait TextResolver: Sync {
    /// Resolves `key` as a Record field stores it, with its leading `@`.
    fn resolve<'a>(&'a self, key: &str) -> Resolution<'a>;

    /// Resolves the key and retains the original lookup when available.
    fn resolve_observed<'a>(&'a self, key: &str) -> ObservedResolution<'a> {
        ObservedResolution::unobserved(self.resolve(key))
    }
}

pub(crate) fn observed_annotation<'t>(
    annotations: &Annotations<'t>,
    key: &str,
) -> Option<ObservedResolution<'t>> {
    if key.is_empty() {
        return None;
    }
    let result = annotations.texts()?.resolve_observed(key);
    match result.resolution() {
        Resolution::Keep => None,
        _ => Some(result),
    }
}
