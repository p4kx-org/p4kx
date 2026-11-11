// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::sync::Mutex;

use p4kx_extraction::StructValue;
use thiserror::Error;

use crate::catalog_read::original::ExampleBodyReceipt;

#[path = "template_error_description.rs"]
mod description;
pub(crate) use description::TemplateDescription;

/// A contract template whose structure the catalog does not follow to its end.
#[derive(Debug, Error)]
pub(crate) enum TemplateError {
    /// A field that holds one of several structs holds one the catalog does not know.
    #[error("{field} holds the unknown struct {name}")]
    UnknownStruct {
        /// The field, e.g. `modifiers`.
        field: &'static str,
        /// The struct name.
        name: String,
    },
    #[error("{error}")]
    Captured {
        #[source]
        error: Box<Self>,
        body: ExampleBodyReceipt,
    },
}

impl TemplateError {
    pub(crate) fn with_body(self, body: ExampleBodyReceipt) -> Self {
        Self::Captured {
            error: Box::new(self),
            body,
        }
    }

    pub(crate) const fn body(&self) -> Option<&ExampleBodyReceipt> {
        match self {
            Self::Captured { body, .. } => Some(body),
            Self::UnknownStruct { .. } => None,
        }
    }

    /// Returns the error for the struct `value` in the field `field`.
    pub(crate) fn unknown(field: &'static str, value: &StructValue<'_>) -> Self {
        Self::UnknownStruct {
            field,
            name: value.type_name().to_owned(),
        }
    }
}

/// Counts unsupported vocabulary by technical category.
#[derive(Debug, Default)]
pub(crate) struct Unknowns(Mutex<BTreeMap<Cow<'static, str>, usize>>);

impl Unknowns {
    /// Interprets an authored value and counts an unsupported value by category.
    pub(crate) fn parse<T>(
        &self,
        category: &'static str,
        stored: Option<&str>,
        parse: impl FnOnce(&str) -> Option<T>,
    ) -> Option<T> {
        self.parse_counted(stored?, parse, || Cow::Borrowed(category))
    }

    /// Interprets a declared source type and retains its technical name when unsupported.
    pub(crate) fn parse_type<T>(
        &self,
        category: &'static str,
        source_type: &str,
        parse: impl FnOnce(&str) -> Option<T>,
    ) -> Option<T> {
        self.parse_counted(source_type, parse, || {
            Cow::Owned(format!("{category} {source_type}"))
        })
    }

    fn parse_counted<T>(
        &self,
        stored: &str,
        parse: impl FnOnce(&str) -> Option<T>,
        label: impl FnOnce() -> Cow<'static, str>,
    ) -> Option<T> {
        let parsed = parse(stored);
        if parsed.is_none()
            && let Ok(mut counts) = self.0.lock()
        {
            let count = counts.entry(label()).or_insert(0);
            *count = count.saturating_add(1);
        }
        parsed
    }

    /// Returns the total count and ordered counts by technical category.
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
