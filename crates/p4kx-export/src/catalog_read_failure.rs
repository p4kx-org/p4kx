// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{DataForgeError, Guid, Record};

use crate::CatalogFailure;

/// A catalog read failure retaining its original Record until header capture.
#[derive(Clone)]
pub(crate) struct CatalogReadFailure<'source> {
    record: Record<'source>,
    canonical: Option<Box<DataForgeError>>,
    record_path: &'source str,
    reason: String,
}

impl<'source> CatalogReadFailure<'source> {
    pub(crate) fn of(record: &Record<'source>, error: &(dyn Error + 'static)) -> Self {
        Self {
            record: *record,
            canonical: canonical(error),
            record_path: record.record_path().unwrap_or_default(),
            reason: crate::failure_text::render(error),
        }
    }

    pub(crate) fn of_loadout(record: &Record<'source>, error: &(dyn Error + 'static)) -> Self {
        let mut failure = Self::of(record, error);
        failure.reason.insert_str(0, "loadout: ");
        failure
    }

    pub(crate) const fn guid(&self) -> Guid {
        self.record.guid()
    }

    pub(crate) const fn reason(&self) -> &str {
        self.reason.as_str()
    }

    pub(crate) const fn record(&self) -> &Record<'source> {
        &self.record
    }

    pub(crate) fn canonical_error(&self) -> Option<&DataForgeError> {
        self.canonical.as_deref()
    }

    pub(crate) fn into_interpreted(self) -> CatalogFailure {
        CatalogFailure {
            guid: self.guid(),
            record_path: self.record_path.to_owned(),
            reason: self.reason,
        }
    }
}
fn canonical(error: &(dyn Error + 'static)) -> Option<Box<DataForgeError>> {
    let mut cause = Some(error);
    while let Some(error) = cause {
        if let Some(error) = error.downcast_ref::<DataForgeError>() {
            return Some(Box::new(error.clone()));
        }
        cause = error.source();
    }
    None
}
impl std::fmt::Debug for CatalogReadFailure<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CatalogReadFailure")
            .field("guid", &self.guid())
            .field("record_path", &self.record_path)
            .field("reason", &self.reason)
            .finish()
    }
}
impl PartialEq for CatalogReadFailure<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.guid() == other.guid()
            && self.record_path == other.record_path
            && self.reason == other.reason
    }
}

impl Eq for CatalogReadFailure<'_> {}
