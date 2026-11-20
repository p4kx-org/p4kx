// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use serde_json::Value;

use super::TestResult;
use crate::real_archive_catalog_table_parity;

pub(super) struct TextScope<'catalog> {
    texts: HashMap<&'catalog str, &'catalog Value>,
    uses: HashMap<&'catalog str, &'catalog Value>,
}

impl<'catalog> TextScope<'catalog> {
    pub(super) fn read(catalog: &'catalog Value) -> TestResult<Self> {
        Ok(Self {
            texts: index(catalog, "/texts/gameTexts", "/id/value")?,
            uses: index(catalog, "/provenance/uses", "/id")?,
        })
    }

    pub(super) fn for_owner(&self, id: &str, owner: u32) -> TestResult<Value> {
        let text = self
            .texts
            .get(id)
            .ok_or("matching Catalog protected text row")?;
        let mut selected = (*text).clone();
        self.restrict(&mut selected, owner)?;
        Ok(selected)
    }

    fn restrict(&self, value: &mut Value, owner: u32) -> TestResult {
        match value {
            Value::Object(fields) => {
                for (name, field) in fields {
                    if name == "sourceUsesRefs" {
                        self.restrict_refs(field, owner)?;
                    } else {
                        self.restrict(field, owner)?;
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    self.restrict(value, owner)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn restrict_refs(&self, value: &mut Value, owner: u32) -> TestResult {
        let refs = value
            .as_array_mut()
            .ok_or("explicit Catalog protected-text contributors")?;
        let mut kept = Vec::with_capacity(refs.len());
        for used in refs.drain(..) {
            let id = used
                .as_str()
                .ok_or("Catalog protected-text contributor identifier")?;
            if self
                .owner(id)?
                .is_none_or(|record| record == u64::from(owner))
            {
                kept.push(used);
            }
        }
        *refs = kept;
        Ok(())
    }

    fn owner(&self, id: &str) -> TestResult<Option<u64>> {
        let location = self.location(id)?;
        let registration =
            if location.get("type").and_then(Value::as_str) == Some("localizationLookup") {
                let selector = location
                    .get("selector")
                    .and_then(Value::as_str)
                    .ok_or("original locale lookup selector")?;
                self.location(selector)?
            } else {
                location
            };
        if registration.get("type").and_then(Value::as_str) == Some("dataForge") {
            let owner = registration
                .get("recordIndex")
                .and_then(Value::as_u64)
                .ok_or("original text registration Record index")?;
            Ok(Some(owner))
        } else {
            Ok(None)
        }
    }

    fn location(&self, id: &str) -> TestResult<&Value> {
        self.uses
            .get(id)
            .and_then(|row| row.get("location"))
            .ok_or_else(|| "original text contributor location".into())
    }
}

fn index<'catalog>(
    catalog: &'catalog Value,
    pointer: &str,
    identity: &str,
) -> TestResult<HashMap<&'catalog str, &'catalog Value>> {
    let rows = catalog
        .pointer(pointer)
        .and_then(Value::as_array)
        .ok_or("Catalog text proof array")?;
    let mut indexed = HashMap::new();
    for row in rows {
        let id = row
            .pointer(identity)
            .and_then(Value::as_str)
            .ok_or("Catalog text proof identifier")?;
        if indexed.insert(id, row).is_some() {
            return Err("duplicate Catalog text proof identifier".into());
        }
    }
    Ok(indexed)
}

pub(super) fn compare_row(expected: &Value, actual: &Value) -> TestResult {
    if let Some(path) = real_archive_catalog_table_parity::first_difference(expected, actual) {
        return Err(format!("SDK and Catalog FactionIdentity projection differs at {path}").into());
    }
    Ok(())
}

#[test]
fn shared_text_retains_only_locale_uses_for_the_selected_physical_owner() -> TestResult {
    use serde_json::json;

    let catalog = json!({
        "texts": {"gameTexts": [{
            "id": {"value": "text", "sourceUsesRefs": ["first", "alias"]},
            "translations": {"sourceUsesRefs": ["first", "alias", "first-en", "alias-en", "common"]}
        }]},
        "provenance": {"uses": [
            {"id": "first", "location": {"type": "dataForge", "recordIndex": 2}},
            {"id": "alias", "location": {"type": "dataForge", "recordIndex": 9}},
            {"id": "first-en", "location": {"type": "localizationLookup", "selector": "first"}},
            {"id": "alias-en", "location": {"type": "localizationLookup", "selector": "alias"}},
            {"id": "common", "location": {"type": "metadata"}}
        ]}
    });
    let expected = json!({
        "id": {"value": "text", "sourceUsesRefs": ["alias"]},
        "translations": {"sourceUsesRefs": ["alias", "alias-en", "common"]}
    });
    let texts = TextScope::read(&catalog)?;
    assert_eq!(
        texts.for_owner("text", 9)?,
        expected,
        "same-key aliases contribute distinct Record and locale observations"
    );
    Ok(())
}
