// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, Filter, Record};

use crate::export_ledger::{SourceDiagnosis, SourceDisposition};

use super::projection::ProjectionError;
use super::selection;

#[derive(Clone, Copy)]
pub(super) enum Fault<'a> {
    Read(&'a DataForgeError),
    Projection(&'a ProjectionError),
}

impl Fault<'_> {
    pub(super) fn error(&self) -> &(dyn std::error::Error + 'static) {
        match self {
            Self::Read(error) => *error,
            Self::Projection(error) => *error,
        }
    }

    pub(super) const fn disposition(&self) -> SourceDisposition {
        match self {
            Self::Read(_) => SourceDisposition::ReadFailed,
            Self::Projection(_) => SourceDisposition::ProjectionFailed,
        }
    }

    pub(super) fn diagnosis(&self) -> SourceDiagnosis {
        match self {
            Self::Read(error) => crate::export_source_failure::diagnosis(*error),
            Self::Projection(error) => error.diagnosis(),
        }
    }

    pub(super) fn compatibility_failure(
        &self,
        record: &Record<'_>,
        filter: Option<&Filter>,
    ) -> bool {
        match self {
            Self::Read(_) => !selection::outside(record, filter),
            Self::Projection(_) => true,
        }
    }
}
