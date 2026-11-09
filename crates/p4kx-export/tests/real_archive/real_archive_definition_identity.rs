// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_export::Kind;
use serde_json::Value;

use super::{TestResult, list};
use crate::catalog_v3_consumer;

pub(super) fn check(catalog: &Value, kind: Kind) -> TestResult {
    let mut physical = HashSet::new();
    let mut order = Vec::new();
    for row in list(catalog, kind)? {
        let index = catalog_v3_consumer::field(row, kind, "recordIndex")?
            .as_u64()
            .ok_or("original physical Record index")?;
        if !physical.insert(index) {
            return Err("one physical definition cannot occur twice".into());
        }
        let source_type = catalog_v3_consumer::field(row, kind, "sourceType")?
            .as_str()
            .ok_or("definition source type")?;
        let matching = match kind {
            Kind::QualityDistributions => matches!(
                source_type,
                "CraftingQualityDistributionRecord" | "CraftingQualityLocationOverrideRecord"
            ),
            Kind::BlueprintCategories => source_type == "BlueprintCategoryRecord",
            Kind::QualityQuantizations => source_type == "CraftingQualityQuantizationRecord",
            _ => false,
        };
        if !matching {
            return Err("a physical definition row belongs to its declared source family".into());
        }
        order.push((catalog_v3_consumer::identity(row, kind)?, index));
    }
    if !order.is_sorted() {
        return Err("equal nominal GUIDs keep increasing original physical positions".into());
    }
    Ok(())
}
