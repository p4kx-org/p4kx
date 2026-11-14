// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::Value;

use super::{CoverageCounters, TestResult, TextDomain, TextIndex, TextReferenceShape};
use crate::catalog_v3_consumer;

pub(super) fn compare(
    counters: &mut CoverageCounters<'_>,
    index: &TextIndex<'_>,
    kind: Kind,
    row: &Value,
) -> TestResult<()> {
    let (field, domain) = match kind {
        Kind::ResourceGroups | Kind::ReputationScopes => ("description", TextDomain::GameText),
        Kind::ReputationStandings => ("perkDescription", TextDomain::GameText),
        Kind::GameplayProperties => ("unitFormat", TextDomain::PublicName),
        _ => return Ok(()),
    };
    let target = index.reference(
        row.get(field).ok_or("authored text carrier")?,
        domain,
        TextReferenceShape::Required,
    )?;
    counters.count_supplemental(target)?;
    if kind == Kind::GameplayProperties {
        compare_overrides(counters, index, row)?;
    }
    Ok(())
}

fn compare_overrides(
    counters: &mut CoverageCounters<'_>,
    index: &TextIndex<'_>,
    row: &Value,
) -> TestResult<()> {
    let list = row
        .get("nameOverrides")
        .ok_or("authored name override array")?;
    let Some(list) = catalog_v3_consumer::source_value(list)? else {
        return Ok(());
    };
    let items = list.as_array().ok_or("ordered name override array")?;
    for item in items {
        let Some(item) = catalog_v3_consumer::source_value(item)? else {
            continue;
        };
        let body = item.get("payload").ok_or("authored name override body")?;
        let Some(body) = catalog_v3_consumer::source_value(body)? else {
            continue;
        };
        let target = index.reference(
            body.get("propertyName")
                .ok_or("authored property name override")?,
            TextDomain::PublicName,
            TextReferenceShape::Required,
        )?;
        counters.count_supplemental(target)?;
    }
    Ok(())
}
