// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Filter, Guid};

use crate::selection::Kind;

/// The metadata identity of an actual emitted or intentionally excluded catalog row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogRow {
    pub(crate) kind: Kind,
    pub(crate) guid: Guid,
    pub(crate) record_path: String,
}

/// The catalog policy that excluded a successfully projected source row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CatalogRowExclusion {
    Filter,
    DevelopmentLeftover,
}

/// One actual row exclusion applied after a canonical source read.
#[derive(Debug, Clone)]
pub(crate) struct ExcludedCatalogRow {
    pub(crate) row: CatalogRow,
    pub(crate) reason: CatalogRowExclusion,
}

/// Final target rows and the exact Filter applied to their catalog selection.
#[derive(Debug, Clone, Default)]
pub(crate) struct CatalogRows {
    pub(crate) emitted: Vec<CatalogRow>,
    pub(crate) filter: Option<Filter>,
    pub(crate) numbers: Vec<crate::catalog_number_health::EmittedNumber>,
    pub(crate) template_numbers: Vec<crate::template_number_capture::TemplateNumbers>,
    pub(crate) recovery_numbers: Vec<crate::catalog_number_capture::CapturedNumber>,
}
