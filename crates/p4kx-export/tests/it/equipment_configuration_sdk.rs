// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use p4kx_extraction::Archive;
use serde_json::Value;

use super::{Layout, database};
use crate::catalog_item_stats::inventory_entities as entities;
use crate::commodity_v3_fixture::{self as common, TestResult};
use crate::health_origin_assertions as proof;

#[path = "equipment_attachment_walk.rs"]
mod attachment_walk;
#[path = "equipment_attachment_fixture.rs"]
mod attachments;
#[path = "equipment_bounds_fixture.rs"]
mod bounds;
#[path = "equipment_bounds_walk.rs"]
mod bounds_walk;
#[path = "equipment_configuration_failures.rs"]
mod failures;
#[path = "equipment_configuration_parity.rs"]
mod parity;
#[path = "equipment_configuration_selection.rs"]
mod selection;
#[path = "equipment_units_walk.rs"]
mod units;
#[path = "equipment_configuration_visit.rs"]
mod visit;

const ATTACHMENTS: &str = "/configurations/itemAttachmentConstraints";
const BOUNDS: &str = "/configurations/itemBounds";

fn catalog(archive: &Archive, mode: CatalogPublicationMode) -> TestResult<Value> {
    let mut options = common::options();
    options.publication_mode = mode;
    Ok(serde_json::from_slice(
        p4kx_export::build_catalog(archive, options.with_full_provenance())?.json(),
    )?)
}

fn selected_rows<'a>(
    catalog: &'a Value,
    table: &str,
    physical: Option<&[u32]>,
) -> TestResult<Vec<&'a Value>> {
    let rows = proof::rows(catalog, &format!("{table}/rows"))?;
    Ok(rows
        .iter()
        .filter(|row| {
            physical.is_none_or(|selected| {
                row.pointer("/ownerRecordIndex/value")
                    .and_then(Value::as_u64)
                    .is_some_and(|index| {
                        selected
                            .iter()
                            .any(|selected| u64::from(*selected) == index)
                    })
            })
        })
        .collect())
}
