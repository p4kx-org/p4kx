// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use serde_json::Value;

use super::{CoverageCounters, TestResult, TextDomain, TextIndex, TextReferenceShape, TextTarget};

pub(super) fn count(
    texts: &TextIndex<'_>,
    kind: Kind,
    sets: &[Value],
    counters: &mut CoverageCounters<'_>,
) -> TestResult<()> {
    if kind != Kind::MissionPropertySets || sets.is_empty() {
        return Ok(());
    }
    for set in sets {
        for property in super::graph::array(set, "properties")? {
            if property.is_null() {
                continue;
            }
            let value = property.get("value").ok_or("Generator property value")?;
            match value.get("type").and_then(Value::as_str) {
                Some("stringHash" | "integer" | "float" | "tags") => {
                    for option in entries(value, "options")? {
                        count_option(option, texts, counters)?;
                    }
                }
                Some("aiName") => {
                    for field in ["/givenName", "/givenLastName", "/givenNickName"] {
                        let target = required_reference(value, field, texts)?;
                        if target.is_some_and(|target| !is_lookup_key(target.key)) {
                            return Err(format!("invalid direct Generator name at {field}").into());
                        }
                        counters.count(target)?;
                    }
                }
                Some("investigation") => count_investigation(value, texts, counters)?,
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
    for suspect in entries(investigation, "suspects")? {
        count_investigation_name(suspect, texts, counters)?;
    }
    for location in entries(investigation, "locations")? {
        for item in entries(location, "clueItems")? {
            for clue in entries(item, "clues")? {
                for field in ["/objectiveLongDescription", "/objectiveShortDescription"] {
                    counters.count(required_reference(clue, field, texts)?)?;
                }
                let details = clue
                    .get("details")
                    .ok_or("required Generator clue details")?;
                if super::graph::text(details, "/type")? == "corpseScanInfo" {
                    counters.count(required_reference(details, "/causeOfDeath", texts)?)?;
                    count_investigation_name(details, texts, counters)?;
                }
            }
        }
    }
    Ok(())
}

fn count_investigation_name(
    owner: &Value,
    texts: &TextIndex<'_>,
    counters: &mut CoverageCounters<'_>,
) -> TestResult<()> {
    let name = owner
        .get("name")
        .ok_or("required nullable Investigation name")?;
    if name.is_null() {
        return Ok(());
    }
    let source = name
        .get("source")
        .ok_or("required original Investigation name source")?;
    for (member, field) in [
        ("characterGivenName", "givenName"),
        ("characterGivenLastName", "givenLastName"),
        ("characterGivenNickName", "givenNickName"),
    ] {
        let expected = match source.get(member) {
            Some(Value::String(key)) => Some(key.as_str()).filter(|key| is_lookup_key(key)),
            Some(Value::Null) => None,
            _ => return Err(format!("invalid original Investigation name field {member}").into()),
        };
        let carrier = name
            .get(field)
            .ok_or_else(|| format!("required Investigation name field {field}"))?;
        let target =
            texts.reference(carrier, TextDomain::GameText, TextReferenceShape::Required)?;
        if target.map(|target| target.key) != expected
            || (expected.is_none() && carrier["state"] != "absent")
        {
            return Err(
                format!("Investigation name {field} disagrees with its original key").into(),
            );
        }
        counters.count(target)?;
    }
    Ok(())
}

fn required_reference<'index, 'catalog>(
    owner: &Value,
    field: &str,
    texts: &'index TextIndex<'catalog>,
) -> TestResult<Option<&'index TextTarget<'catalog>>> {
    let carrier = owner
        .pointer(field)
        .ok_or_else(|| format!("required Generator text at {field}"))?;
    texts.reference(carrier, TextDomain::GameText, TextReferenceShape::Required)
}

fn is_lookup_key(key: &str) -> bool {
    key.starts_with('@') && !p4kx_extraction::is_placeholder_key(key)
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
        Value::String(key) => Some(key.as_str()).filter(|key| is_lookup_key(key)),
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
