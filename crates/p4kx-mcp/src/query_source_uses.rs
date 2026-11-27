// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::query::QuerySource;
use p4kx_extraction::query_source_evidence::QuerySourceSite;
use p4kx_extraction::{SourceUse, SourceUseRole};

use crate::forge_origins::{EvidenceError, ForgeOrigins};

pub(crate) fn record_site(
    origins: &ForgeOrigins,
    index: u32,
    field: &str,
    role: SourceUseRole,
) -> Result<SourceUse, EvidenceError> {
    Ok(QuerySourceSite::record(index, field, role).bind(&origins.scope)?)
}

pub(crate) fn uses(
    origins: &ForgeOrigins,
    sources: impl IntoIterator<Item = QuerySource>,
    value_role: SourceUseRole,
) -> Result<Vec<SourceUse>, EvidenceError> {
    sources
        .into_iter()
        .filter_map(|source| QuerySourceSite::of(source, value_role))
        .map(|site| site.bind(&origins.scope).map_err(Into::into))
        .collect()
}
