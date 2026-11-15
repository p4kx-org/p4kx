// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::Value;

use super::{CoverageCounters, TestResult, TextDomain, TextIndex, TextReferenceShape};
use crate::catalog_v3_consumer;

pub(super) fn count(
    catalog: &Value,
    contracts: &[Value],
    index: &TextIndex<'_>,
    counters: &mut CoverageCounters<'_>,
) -> TestResult<()> {
    let types = catalog_v3_consumer::rows(catalog, Kind::MissionTypes)?;
    for contract in contracts {
        let guid = contract
            .get("missionTypeGuid")
            .ok_or("contract original type GUID")?;
        let name = contract
            .get("missionType")
            .ok_or("contract type name cell")?;
        let mut matching = types
            .iter()
            .filter(|row| row.get("guid") == Some(guid))
            .peekable();
        if matching.peek().is_some() && !matching.any(|row| row.get("typeName") == Some(name)) {
            return Err("contract name does not match its canonical mission type".into());
        }
        if !name.is_null() {
            if !guid.is_string() {
                return Err("contract type text requires the original type GUID".into());
            }
            counters.count(index.reference(
                name,
                TextDomain::GameText,
                TextReferenceShape::Required,
            )?)?;
        }
    }
    Ok(())
}
