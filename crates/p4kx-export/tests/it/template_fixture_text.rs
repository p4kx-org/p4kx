// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Context;
use crate::template_fixture_fields as fields;

pub(crate) fn localized(
    catalog: &Value,
    graph: &dyn Context<'_>,
    node: &Value,
    key: &str,
    english: Option<&str>,
    german: Option<&str>,
) -> TestResult {
    assert_eq!(
        node.pointer("/value/value/value/type"),
        Some(&json!("gameText")),
        "the original localized token retains its protected text domain"
    );
    assert_eq!(
        fields::text(node)?,
        key,
        "the complete original lookup token"
    );
    let reference = graph
        .owner()
        .reference(crate::flow_owner_graph::ordinal(node)?)?;
    let selected: Vec<_> = raw::array(graph.projection(), "localizations")?
        .iter()
        .filter(|row| row.get("source") == Some(&reference))
        .collect();
    assert_eq!(
        selected.len(),
        1,
        "one real lookup for this same original occurrence"
    );
    let selected = *selected.first().ok_or("actual contextual text lookup")?;
    assert_eq!(
        selected.get("interpretation"),
        Some(&json!("lookup")),
        "the original key is recognized separately from source presence"
    );
    let identifier = raw::present(selected.get("text").ok_or("actual GameTextRef")?)?;
    let matching: Vec<_> = catalog
        .pointer("/texts/gameTexts")
        .and_then(Value::as_array)
        .ok_or("canonical protected text rows")?
        .iter()
        .filter(|row| row.pointer("/id/value") == Some(identifier))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "the actual typed lookup resolves in this catalog"
    );
    let row = *matching.first().ok_or("selected original canonical text")?;
    assert_eq!(
        raw::present(row.get("key").ok_or("canonical key cell")?)?,
        &json!(key),
        "the original key is preserved independently"
    );
    assert_languages(row, english, german)
}

fn assert_languages(row: &Value, english: Option<&str>, german: Option<&str>) -> TestResult {
    let languages = raw::present(
        row.get("translations")
            .ok_or("actual available languages")?,
    )?;
    for (language, expected) in [("en", english), ("de", german)] {
        match expected {
            Some(expected) => assert_eq!(
                raw::present(
                    languages
                        .get(language)
                        .and_then(|row| row.get("sourceText"))
                        .ok_or("actual available source translation")?
                )?,
                &json!(expected),
                "the exact original language text remains"
            ),
            None => assert!(
                languages.get(language).is_none(),
                "an unavailable original translation is not fabricated as a text value"
            ),
        }
    }
    Ok(())
}

pub(crate) fn no_lookup(
    graph: &dyn Context<'_>,
    node: &Value,
    original: &str,
    interpretation: &str,
) -> TestResult {
    assert_eq!(
        node.pointer("/value/value/value/type"),
        Some(&json!("gameText")),
        "the original localized token retains its protected text domain"
    );
    assert_eq!(
        fields::text(node)?,
        original,
        "the complete original inert token"
    );
    let reference = graph
        .owner()
        .reference(crate::flow_owner_graph::ordinal(node)?)?;
    let matching: Vec<_> = raw::array(graph.projection(), "localizations")?
        .iter()
        .filter(|row| row.get("source") == Some(&reference))
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "the same actual source occurrence has one interpretation"
    );
    let row = *matching.first().ok_or("actual no-lookup interpretation")?;
    assert_eq!(
        row.get("interpretation"),
        Some(&json!(interpretation)),
        "the established lookup decision"
    );
    let text = row
        .get("text")
        .ok_or("required independently sourced lookup result")?;
    assert_eq!(
        text.get("state"),
        Some(&json!("absent")),
        "no fabricated translated key"
    );
    assert_eq!(
        text.get("value"),
        Some(&Value::Null),
        "the absent lookup has no text payload"
    );
    Ok(())
}
