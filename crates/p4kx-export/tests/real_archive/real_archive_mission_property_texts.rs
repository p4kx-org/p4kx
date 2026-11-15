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
            match value.get("type").and_then(Value::as_str) {
                Some("stringHash" | "integer" | "float" | "tags") => {
                    for option in entries(value, "options")? {
                        count_option(option, &texts, counters)?;
                    }
                }
                Some("investigation") => count_investigation(value, &texts, counters)?,
                _ => {}
            }
        }
    }
    Ok(())
}

fn count_investigation(
    investigation: &Value,
    texts: &TextIndex<'_>,
    counters: &mut CoverageCounters<'_>,
) -> TestResult<()> {
    for location in entries(investigation, "locations")? {
        for item in entries(location, "clueItems")? {
            for clue in entries(item, "clues")? {
                for field in ["/objectiveLongDescription", "/objectiveShortDescription"] {
                    count_clue_text(clue, field, texts, counters)?;
                }
                if super::graph::text(clue, "/details/type")? == "corpseScanInfo" {
                    count_clue_text(clue, "/details/causeOfDeath", texts, counters)?;
                }
            }
        }
    }
    Ok(())
}

fn count_clue_text(
    clue: &Value,
    field: &str,
    texts: &TextIndex<'_>,
    counters: &mut CoverageCounters<'_>,
) -> TestResult<()> {
    let carrier = clue
        .pointer(field)
        .ok_or_else(|| format!("required Generator clue text at {field}"))?;
    let target = texts.reference(carrier, TextDomain::GameText, TextReferenceShape::Required)?;
    counters.count(target)
}

fn entries<'value>(
    node: &'value Value,
    field: &str,
) -> TestResult<impl Iterator<Item = &'value Value>> {
    let entries = match node.get(field) {
        Some(Value::Array(entries)) => entries.as_slice(),
        Some(Value::Null) => &[],
        _ => return Err(format!("required nullable Generator array {field}").into()),
    };
    Ok(entries.iter().filter(|entry| !entry.is_null()))
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
