// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{CancelToken, TreeBuilder};

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_query_session::checkpoint;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::definition_reference::DefinitionStatus;
use crate::mining_locations::{MineableElement, MiningLocationCoverage, MiningResourceType};
use crate::record_observation::RecordObservation;
use crate::resource_read;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(super) fn read<'source>(
    projection: &CatalogProjection<'source, 'source>,
    elements: &[MineableElement],
    cancel: Option<&CancelToken>,
    coverage: &mut MiningLocationCoverage,
) -> Result<Vec<MiningResourceType>, CatalogError> {
    let projection = projection.for_kind(Kind::ResourceTypes);
    let ore_positions: BTreeSet<_> = elements
        .iter()
        .filter_map(|element| {
            bound_target(element.resource_type.value().map(Box::as_ref), coverage)
        })
        .collect();
    let mut refined_positions = BTreeSet::new();
    let mut trees = TreeBuilder::new();
    let mut rows = Vec::new();
    for &position in &ore_positions {
        if let Some(row) = read_position(&projection, &mut trees, position, cancel, coverage)? {
            if let Some(refined) = bound_target(row.refined_version.value(), coverage)
                && !ore_positions.contains(&refined)
            {
                refined_positions.insert(refined);
            }
            rows.push(row);
        }
    }
    for position in refined_positions {
        if let Some(row) = read_position(&projection, &mut trees, position, cancel, coverage)? {
            rows.push(row);
        }
    }
    rows.sort_by_cached_key(|row| row.source.order_key());
    Ok(rows)
}

fn bound_target(
    link: Option<&CatalogRecordReference>,
    coverage: &mut MiningLocationCoverage,
) -> Option<u32> {
    let reference = link?;
    if matches!(reference.status(), DefinitionStatus::Present)
        && reference
            .target_type()
            .value()
            .is_some_and(|name| name == Kind::ResourceTypes.records().type_name)
        && let Some(&position) = reference.target_record_index().value()
    {
        return Some(position);
    }
    coverage.unresolved_resource_links += 1;
    None
}

fn read_position<'source>(
    projection: &CatalogProjection<'source, 'source>,
    trees: &mut TreeBuilder<'source>,
    position: u32,
    cancel: Option<&CancelToken>,
    coverage: &mut MiningLocationCoverage,
) -> Result<Option<MiningResourceType>, CatalogError> {
    checkpoint(cancel)?;
    let input = projection.original();
    let record = input
        .forge()
        .record(position)
        .ok_or(ExportAccountingError::MissingPhysicalReferenceTarget { index: position })?;
    let Ok(tree) = trees.build(&record) else {
        coverage.unreadable_resource_types += 1;
        return Ok(None);
    };
    let admission = RecordObservation::capture(input, &record)?;
    let (value, notes) =
        resource_read::read_name_and_refinement(&tree, projection, &admission, cancel)?;
    coverage.unreadable_mining_fields += notes.len();
    Ok(Some(MiningResourceType {
        source: CanonicalRecordHeader::read(&tree, input.origins())?.into(),
        name: value.name,
        refined_version: value.refined_version,
    }))
}

#[cfg(test)]
#[path = "mining_resource_rows_tests.rs"]
mod tests;
