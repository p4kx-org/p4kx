// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_extraction::Language;
use serde_json::Value;

use super::schema_navigation::{escape, resolve};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

const RELEASED: &str = include_str!("../fixtures/catalog-v0.4.2.schema.json");
const CATEGORY: &str = "blueprintCategories";
const PROPERTY: &str = "gameplayProperties";
const HEADER: &[&str] = &["/className", "/guid", "/recordPath"];
const NAME_FIELDS: &[&str] = &["/name", "/name/key", "/name/en", "/name/de"];

struct Mapping {
    old: &'static str,
    disposition: Disposition,
}

enum Disposition {
    SourcedHeader,
    PublicName,
    AuthoredKey,
    DisplayText(Language),
    RemovedEmptyName,
}

fn category_mappings() -> Vec<Mapping> {
    let mut fields = header_mappings();
    fields.extend(NAME_FIELDS.iter().map(|old| Mapping {
        old,
        disposition: Disposition::RemovedEmptyName,
    }));
    fields
}

fn property_mappings() -> Vec<Mapping> {
    let mut fields = header_mappings();
    fields.extend([
        Mapping {
            old: "/name",
            disposition: Disposition::PublicName,
        },
        Mapping {
            old: "/name/key",
            disposition: Disposition::AuthoredKey,
        },
        Mapping {
            old: "/name/en",
            disposition: Disposition::DisplayText(Language::English),
        },
        Mapping {
            old: "/name/de",
            disposition: Disposition::DisplayText(Language::German),
        },
    ]);
    fields
}

fn header_mappings() -> Vec<Mapping> {
    HEADER
        .iter()
        .map(|old| Mapping {
            old,
            disposition: Disposition::SourcedHeader,
        })
        .collect()
}

#[test]
fn released_v2_category_and_gameplay_property_fields_have_explicit_dispositions() -> Result {
    let old: Value = serde_json::from_str(RELEASED)?;
    let current = p4kx_export::catalog_schema();
    assert_eq!(old["properties"]["schemaVersion"]["const"], 2);
    for (family, rows) in [
        (CATEGORY, category_mappings()),
        (PROPERTY, property_mappings()),
    ] {
        let discovered = old_fields(&old, family)?;
        let classified: BTreeSet<_> = rows.iter().map(|row| row.old.to_owned()).collect();
        assert_eq!(
            classified.len(),
            rows.len(),
            "one migration disposition per {family} field"
        );
        assert_eq!(
            discovered, classified,
            "released {family} migration coverage"
        );
        for row in &rows {
            verify_destination(&current, family, row)?;
        }
    }
    Ok(())
}

#[test]
fn an_unclassified_released_field_is_reported_by_the_inventory() -> Result {
    let mut old: Value = serde_json::from_str(RELEASED)?;
    old["$defs"]["BlueprintCategory"]["properties"]["fixtureAdditionalField"] =
        serde_json::json!({"type": "boolean"});
    let discovered = old_fields(&old, CATEGORY)?;
    let classified: BTreeSet<_> = category_mappings()
        .into_iter()
        .map(|row| row.old.to_owned())
        .collect();
    let unknown: Vec<_> = discovered
        .difference(&classified)
        .map(String::as_str)
        .collect();
    assert_eq!(unknown, ["/fixtureAdditionalField"]);
    Ok(())
}

fn verify_destination(schema: &Value, family: &str, row: &Mapping) -> Result {
    let base = format!("/definitions/{family}/rows/[]");
    let location = match row.disposition {
        Disposition::SourcedHeader => format!("{base}{}/value", row.old),
        Disposition::PublicName => {
            schema_at(schema, "/texts/publicNames/[]/id/value")?;
            format!("{base}/propertyName/value")
        }
        Disposition::AuthoredKey => "/texts/publicNames/[]/key/value".to_owned(),
        Disposition::DisplayText(language) => {
            assert_eq!(
                row.old,
                format!("/name/{}", language.code()),
                "the old field maps to its own language"
            );
            "/texts/publicNames/[]/translations/value/*/displayText/value".to_owned()
        }
        Disposition::RemovedEmptyName => {
            let fields = schema_at(schema, &base)?;
            assert!(
                fields.pointer("/properties/name").is_none(),
                "the released category name had no authored source"
            );
            return Ok(());
        }
    };
    let destination = schema_at(schema, &location)?;
    assert_eq!(
        destination.get("type").and_then(Value::as_str),
        Some("string"),
        "{family}{} → {location}",
        row.old
    );
    Ok(())
}

fn schema_at<'schema>(schema: &'schema Value, path: &str) -> Result<&'schema Value> {
    let mut node = schema;
    for segment in path.trim_start_matches('/').split('/') {
        node = resolve(schema, node)?;
        let child = match segment {
            "[]" => node.get("items"),
            "*" => node.get("additionalProperties"),
            "value" if node.pointer("/properties/value").and_then(Value::as_bool) == Some(true) => {
                present_value(node)
            }
            name => node
                .get("properties")
                .and_then(|fields| fields.get(name.replace("~1", "/").replace("~0", "~"))),
        };
        node = child.ok_or_else(|| format!("missing schema segment {segment} in {path}"))?;
    }
    resolve(schema, node)
}

fn present_value(node: &Value) -> Option<&Value> {
    node.get("allOf")?
        .as_array()?
        .iter()
        .find(|case| {
            case.pointer("/if/properties/state/const")
                .and_then(Value::as_str)
                == Some("present")
        })?
        .pointer("/then/properties/value")
}

fn old_fields(schema: &Value, family: &str) -> Result<BTreeSet<String>> {
    let mut fields = BTreeSet::new();
    let rows = schema_at(schema, &format!("/{family}/[]"))?;
    collect_declarations(schema, rows, "", &mut Vec::new(), &mut fields)?;
    Ok(fields)
}

fn collect_declarations<'schema>(
    schema: &'schema Value,
    node: &'schema Value,
    path: &str,
    active: &mut Vec<&'schema Value>,
    fields: &mut BTreeSet<String>,
) -> Result {
    let node = resolve(schema, node)?;
    if active.iter().any(|prior| std::ptr::eq(*prior, node)) {
        return Ok(());
    }
    if ["allOf", "anyOf", "oneOf"]
        .into_iter()
        .any(|keyword| node.get(keyword).is_some())
    {
        return Err(format!("migration classification needs branch-aware paths at {path}").into());
    }
    active.push(node);
    if let Some(properties) = node.get("properties").and_then(Value::as_object) {
        for (name, child) in properties {
            let path = format!("{path}/{}", escape(name));
            fields.insert(path.clone());
            collect_declarations(schema, child, &path, active, fields)?;
        }
    }
    for (schema_key, suffix) in [("items", "[]"), ("additionalProperties", "*")] {
        if let Some(child) = node.get(schema_key).filter(|value| value.is_object()) {
            collect_declarations(schema, child, &format!("{path}/{suffix}"), active, fields)?;
        }
    }
    active.pop();
    Ok(())
}
