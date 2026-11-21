// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_export::Kind;
use serde_json::Value;

use super::super::{definition_identity, list};
use crate::catalog_v3_consumer;
use crate::commodity_v3_fixture::TestResult;

#[path = "real_archive_item_header_uses.rs"]
mod header_uses;

use header_uses::HeaderUses;

pub(super) fn check(catalog: &Value) -> TestResult {
    definition_identity::check(catalog, Kind::Items)?;
    let definitions = catalog_v3_consumer::table(catalog, "/definitions/items")?;
    definition_identity::check_rows(definitions, Kind::Items)?;
    let projections = list(catalog, Kind::Items)?;
    if projections.len() != definitions.len() {
        return Err("Item projections and original physical definitions must pair exactly".into());
    }
    let headers = HeaderUses::read(catalog, projections.iter().chain(definitions))?;
    let mut by_index = HashMap::new();
    for definition in definitions {
        let key = headers.key(definition)?;
        if by_index.insert(key, definition).is_some() {
            return Err("one original physical Item definition cannot occur twice".into());
        }
    }
    for projection in projections {
        let key = headers.key(projection)?;
        let definition = by_index.get(&key).ok_or("exact physical Item definition")?;
        for field in [
            "guid",
            "recordName",
            "className",
            "recordPath",
            "recordIndex",
            "sourceType",
            "fileName",
            "metadataTag",
        ] {
            if projection.get(field) != definition.get(field) {
                return Err(
                    "the Item projection carries a different original source header".into(),
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "real_archive_item_identity_tests.rs"]
mod tests;
