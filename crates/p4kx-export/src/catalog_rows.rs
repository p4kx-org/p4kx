// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Filter, Guid};

use crate::catalog_row::{CatalogRow, CatalogRows};
use crate::collected::Collected;
use crate::selection::Kind;

/// Captures the actual final row selection independently of successful source projections.
pub(crate) fn snapshot(collected: &Collected<'_>, filter: Option<&Filter>) -> CatalogRows {
    let mut emitted = Vec::new();
    append(
        &mut emitted,
        Kind::Manufacturers,
        collected
            .manufacturers
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    append(
        &mut emitted,
        Kind::Tags,
        collected
            .tags
            .definitions
            .iter()
            .map(|row| (row.source.physical.guid, row.source.physical.record_path)),
    );
    append_resources(&mut emitted, collected);
    append(
        &mut emitted,
        Kind::StarmapObjects,
        collected
            .starmap_objects
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    CatalogRows {
        emitted,
        filter: filter.cloned(),
    }
}

fn append_resources(emitted: &mut Vec<CatalogRow>, collected: &Collected<'_>) {
    append(
        emitted,
        Kind::Commodities,
        collected
            .commodities
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    append(
        emitted,
        Kind::ResourceTypes,
        collected
            .resource_types
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    append(
        emitted,
        Kind::ResourceGroups,
        collected
            .resource_groups
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    append(
        emitted,
        Kind::QualityDistributions,
        collected
            .quality_distributions
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
    append(
        emitted,
        Kind::QualityQuantizations,
        collected
            .quality_quantizations
            .iter()
            .map(|row| (row.guid.0, row.record_path)),
    );
}

fn append<'source>(
    emitted: &mut Vec<CatalogRow>,
    kind: Kind,
    rows: impl Iterator<Item = (Guid, &'source str)>,
) {
    emitted.extend(rows.map(|(guid, record_path)| CatalogRow {
        kind,
        guid,
        record_path: record_path.to_owned(),
    }));
}
