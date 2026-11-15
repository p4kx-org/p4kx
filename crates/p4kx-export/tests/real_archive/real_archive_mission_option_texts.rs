// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::Value;

use super::{CoverageCounters, TestResult, TextDomain, TextIndex, TextReferenceShape};

pub(super) fn count(
    catalog: &Value,
    kind: Kind,
    sets: &[Value],
    counters: &mut CoverageCounters<'_>,
) -> TestResult<()> {
    if kind != Kind::MissionPropertySets || sets.is_empty() {
        return Ok(());
    }
    let texts = TextIndex::new(catalog)?;
    for set in sets {
        for property in super::graph::array(set, "properties")? {
            if property.is_null() {
                continue;
            }
            let value = property.get("value").ok_or("Generator property value")?;
            if !matches!(
                value.get("type").and_then(Value::as_str),
                Some("stringHash" | "integer" | "float" | "tags"),
            ) {
                continue;
            }
            let options = value.get("options").ok_or("Generator property options")?;
            if options.is_null() {
                continue;
            }
            for option in options.as_array().ok_or("Generator option array")? {
                if !option.is_null() {
                    count_option(option, &texts, counters)?;
                }
            }
        }
    }
    Ok(())
}

fn count_option(
    option: &Value,
    texts: &TextIndex<'_>,
    counters: &mut CoverageCounters<'_>,
) -> TestResult<()> {
    let key = match option
        .get("textId")
        .ok_or("original Generator option textId")?
    {
        Value::String(key) => Some(key.as_str())
            .filter(|key| key.starts_with('@') && !p4kx_extraction::is_placeholder_key(key)),
        Value::Null => None,
        _ => return Err("invalid original Generator option textId".into()),
    };
    let target = texts.reference(
        option
            .get("displayName")
            .ok_or("Generator option text reference")?,
        TextDomain::GameText,
        TextReferenceShape::Required,
    )?;
    match (key, target) {
        (Some(key), Some(target)) if key == target.key => counters.count(Some(target)),
        (None, None) => Ok(()),
        _ => Err("Generator option localization disagrees with its original key".into()),
    }
}
