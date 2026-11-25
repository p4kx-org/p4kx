// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::ProvenanceMode;
use serde::Serialize;

use crate::contract::Catalog;

pub(crate) const MARKER_FORMAT: &str = "p4kx-catalog-markers";
pub(crate) const MARKER_FORMAT_VERSION: u32 = 1;

#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum CatalogDocument<'catalog, 'source> {
    Full(&'catalog Catalog<'source>),
    Markers(MarkerDocument<'catalog, 'source>),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MarkerDocument<'catalog, 'source> {
    format: &'static str,
    format_version: u32,
    catalog: &'catalog Catalog<'source>,
}

impl<'catalog, 'source> CatalogDocument<'catalog, 'source> {
    pub(crate) const fn catalog(&self) -> &'catalog Catalog<'source> {
        match self {
            Self::Full(catalog) => catalog,
            Self::Markers(document) => document.catalog,
        }
    }

    pub(crate) fn of(catalog: &'catalog Catalog<'source>, mode: ProvenanceMode) -> Self {
        if mode == ProvenanceMode::MarkersOnly {
            Self::Markers(MarkerDocument {
                format: MARKER_FORMAT,
                format_version: MARKER_FORMAT_VERSION,
                catalog,
            })
        } else {
            Self::Full(catalog)
        }
    }
}
