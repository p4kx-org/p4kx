// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, Language};

use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_query_session::CatalogQuerySource;

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: CatalogQuerySource<'query>,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) selection: CatalogRecordSelection,
    pub(super) languages: &'query [Language],
    pub(super) cancel: Option<&'query CancelToken>,
}
