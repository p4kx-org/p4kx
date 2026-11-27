// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::shops::visit_shops_with_provenance_mode;
use p4kx_export::{CatalogError, StarmapTraversalBudget};
use p4kx_extraction::Archive;
use serde_json::{Value, json};

use super::TestResult;
use crate::health_origin_assertions as health;
use crate::sdk_full_proof as full_proof;

pub(super) fn assert_scope(archive: &Archive, catalog: &Value) -> TestResult {
    let mut rows = Vec::new();
    let all = visit_shops_with_provenance_mode::<CatalogError>(
        archive,
        StarmapTraversalBudget::default(),
        full_proof::MODE,
        None,
        |shop| {
            rows.push(serde_json::to_value(shop).map_err(CatalogError::Json)?);
            Ok(())
        },
    )?;
    assert_eq!(
        all.coverage.inventory_payloads, 1,
        "compatibility opens only the last physical file"
    );
    assert_eq!(
        &json!(rows),
        &catalog["shops"],
        "duplicate inventory preserves the Normal array"
    );
    health::closure(&json!({"provenance": all.source_inventory, "shops": rows}))
}
