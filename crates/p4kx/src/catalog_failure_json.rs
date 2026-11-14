// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogFailureView, HealthText, HealthTextRef};
use p4kx_extraction::SourcedValue;
use serde::Serialize;
#[derive(Serialize)]
pub(super) struct FailureJson<'failure> {
    guid: &'failure SourcedValue<HealthText>,
    path: &'failure SourcedValue<HealthTextRef>,
    error: &'failure SourcedValue<HealthTextRef>,
}

impl<'failure> From<CatalogFailureView<'failure>> for FailureJson<'failure> {
    fn from(failure: CatalogFailureView<'failure>) -> Self {
        let details = failure.details();
        Self {
            guid: details.guid(),
            path: details.record_path(),
            error: details.reason(),
        }
    }
}
