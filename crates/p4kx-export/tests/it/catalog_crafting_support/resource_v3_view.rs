// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Resource semantic comparison views for the crafting integration tests.

use serde_json::{Map, Value, json};

type ViewResult<T> = Result<T, &'static str>;

/// Projects resource source wrappers into the semantic comparison view used by these tests.
pub(super) fn project(mut catalog: Value) -> ViewResult<Value> {
    if catalog.pointer("/definitions/resourceTypes").is_none() {
        return Ok(catalog);
    }
    let rows = catalog
        .pointer("/definitions/resourceTypes/rows")
        .and_then(Value::as_array)
        .ok_or("canonical resource rows are missing")?;
    let texts = catalog
        .get("texts")
        .ok_or("canonical resource texts are missing")?;
    let rows = rows
        .iter()
        .map(|row| resource(row, texts))
        .collect::<ViewResult<Vec<_>>>()?;
    let object = catalog.as_object_mut().ok_or("catalog is not an object")?;
    object
        .get_mut("definitions")
        .and_then(Value::as_object_mut)
        .ok_or("catalog definitions are not an object")?
        .remove("resourceTypes");
    object.remove("provenance");
    object.insert("resourceTypes".to_owned(), Value::Array(rows));
    Ok(catalog)
}

fn resource(row: &Value, texts: &Value) -> ViewResult<Value> {
    let name = localized(row.get("name"), texts, "publicNames")?;
    let description = localized(row.get("description"), texts, "gameTexts")?;
    let mut result = value(row, None)?;
    let object = result
        .as_object_mut()
        .ok_or("resource row is not an object")?;
    object.insert("name".to_owned(), name);
    object.insert("description".to_owned(), description);
    Ok(result)
}

fn localized(reference: Option<&Value>, texts: &Value, domain: &str) -> ViewResult<Value> {
    let Some(reference) = reference else {
        return Ok(empty_text());
    };
    let reference = value(reference, None)?;
    if reference.is_null() {
        return Ok(empty_text());
    }
    let reference = reference.as_str().ok_or("text reference is not a string")?;
    let rows = texts
        .get(domain)
        .and_then(Value::as_array)
        .ok_or("text domain is missing")?;
    let row = rows
        .iter()
        .find(|row| row.pointer("/id/value").and_then(Value::as_str) == Some(reference))
        .ok_or("text reference has no canonical row")?;
    let key = value(row.get("key").ok_or("text key is missing")?, None)?;
    let translations = row
        .pointer("/translations/value")
        .ok_or("translation map is missing")?;
    let translated = |language| {
        let text = translations
            .get(language)
            .map_or(Ok(Value::Null), |text| value(text, None))?;
        Ok::<_, &'static str>(if text.as_str().is_some_and(str::is_empty) {
            Value::Null
        } else {
            text
        })
    };
    Ok(json!({"key": key, "en": translated("en")?, "de": translated("de")?}))
}

fn empty_text() -> Value {
    json!({"key": null, "en": null, "de": null})
}

fn value(input: &Value, field: Option<&str>) -> ViewResult<Value> {
    if sourced(input) {
        if input.get("state").and_then(Value::as_str) == Some("unsupported")
            && field == Some("payload")
        {
            return Ok(json!({"type": "unsupported"}));
        }
        return value(input.get("value").ok_or("source value is missing")?, field);
    }
    if let Some(number) = number(input)? {
        return Ok(number);
    }
    match input {
        Value::Array(values) => Ok(Value::Array(
            values
                .iter()
                .map(|entry| value(entry, None))
                .collect::<ViewResult<_>>()?,
        )),
        Value::Object(fields) => {
            let mut projected: Map<String, Value> = fields
                .iter()
                .map(|(name, entry)| {
                    value(entry, Some(name.as_str())).map(|entry| (name.clone(), entry))
                })
                .collect::<ViewResult<_>>()?;
            if projected.contains_key("start")
                && projected.contains_key("end")
                && projected.contains_key("mappedValue")
                && projected.contains_key("sourceType")
            {
                if projected.get("sourceType").and_then(Value::as_str)
                    != Some("CraftingQualityQuantizationBand")
                {
                    return Ok(Value::Null);
                }
                projected.remove("sourceType");
            }
            Ok(Value::Object(projected))
        }
        primitive => Ok(primitive.clone()),
    }
}

fn sourced(input: &Value) -> bool {
    matches!(
        input.get("state").and_then(Value::as_str),
        Some("present" | "absent" | "null" | "unreadable" | "unsupported" | "invalid")
    ) && input.get("value").is_some()
        && input
            .get("containsEncryptedSource")
            .is_some_and(|flag| flag.is_null() || flag.is_boolean())
        && input.get("sourceUsesRefs").is_some_and(Value::is_array)
        && input
            .get("sourceSetComplete")
            .is_some_and(Value::is_boolean)
}

fn number(input: &Value) -> ViewResult<Option<Value>> {
    let Some(fields) = input.as_object() else {
        return Ok(None);
    };
    if fields
        .get("rawBits")
        .and_then(Value::as_str)
        .is_some_and(|bits| {
            matches!(bits.len(), 8 | 16) && bits.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        && fields
            .get("finite")
            .is_some_and(|finite| finite.is_number() || finite.is_null())
    {
        return Ok(fields.get("finite").cloned());
    }
    if fields.len() != 2 {
        return Ok(None);
    }
    match fields.get("type").and_then(Value::as_str) {
        Some("int8" | "int16" | "int32" | "uint8" | "uint16" | "uint32") => Ok(fields
            .get("value")
            .filter(|value| value.is_number())
            .cloned()),
        Some("int64" | "uint64") => {
            let digits = fields
                .get("value")
                .and_then(Value::as_str)
                .ok_or("wide integer digits are missing")?;
            serde_json::from_str(digits)
                .map(Some)
                .map_err(|_| "wide integer digits are invalid")
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{project, value};

    fn sourced(value: &Value) -> Value {
        json!({"state": "present", "value": value, "containsEncryptedSource": false,
            "sourceUsesRefs": [], "sourceSetComplete": true})
    }

    #[test]
    fn ordinary_state_value_objects_are_not_source_wrappers() -> Result<(), &'static str> {
        let ordinary = json!({"state": "present", "value": false, "other": 0});
        assert_eq!(value(&ordinary, None)?, ordinary);
        assert_eq!(value(&sourced(&json!(false)), None)?, false);
        assert_eq!(value(&sourced(&json!(0)), None)?, 0);
        assert_eq!(value(&sourced(&json!("")), None)?, "");
        Ok(())
    }

    #[test]
    fn source_shapes_and_raw_numbers_have_explicit_comparison_projections()
    -> Result<(), &'static str> {
        let unsupported = json!({"state": "unsupported", "value": null,
            "containsEncryptedSource": null, "sourceUsesRefs": [], "sourceSetComplete": true});
        assert_eq!(
            value(&unsupported, Some("payload"))?,
            json!({"type": "unsupported"})
        );
        assert_eq!(value(&unsupported, Some("field"))?, Value::Null);
        let negative_zero = sourced(&json!({"rawBits": "80000000", "finite": -0.0}));
        assert_eq!(value(&negative_zero, None)?.to_string(), "-0.0");
        let double =
            sourced(&json!({"type": "f64", "rawBits": "3ff4000000000000", "finite": 1.25}));
        assert_eq!(value(&double, None)?, 1.25);
        let wide = sourced(&json!({"type": "uint64", "value": "18446744073709551615"}));
        assert_eq!(value(&wide, None)?, json!(u64::MAX));
        Ok(())
    }

    #[test]
    fn band_projection_retains_source_order_duplicates_and_nulls() -> Result<(), &'static str> {
        let band = json!({"sourceType": sourced(&json!("CraftingQualityQuantizationBand")),
            "start": sourced(&json!({"rawBits": "00000000", "finite": 0.0})),
            "end": sourced(&json!({"rawBits": "3f800000", "finite": 1.0})),
            "mappedValue": sourced(&json!({"rawBits": "3f000000", "finite": 0.5}))});
        let unsupported = json!({"sourceType": sourced(&json!("OtherBand")),
            "start": sourced(&Value::Null), "end": sourced(&Value::Null),
            "mappedValue": sourced(&Value::Null)});
        let actual = value(
            &sourced(&json!([band.clone(), null, unsupported, band])),
            None,
        )?;
        let expected = json!({"start": 0.0, "end": 1.0, "mappedValue": 0.5});
        assert_eq!(actual, json!([expected.clone(), null, null, expected]));
        Ok(())
    }

    #[test]
    fn present_empty_text_keeps_the_legacy_no_display_view() -> Result<(), &'static str> {
        let actual = json!({
            "definitions": {"resourceTypes": {"rows": [{"name": sourced(&json!("name-ref"))}]}},
            "texts": {"publicNames": [{
                "id": sourced(&json!("name-ref")), "key": sourced(&json!("@empty")),
                "translations": sourced(&json!({"en": sourced(&json!(""))}))
            }], "gameTexts": []}
        });
        let comparison = project(actual)?;
        assert_eq!(
            comparison["resourceTypes"][0]["name"],
            json!({"key": "@empty", "en": null, "de": null}),
            "source presence does not change the old empty-display projection"
        );
        Ok(())
    }

    #[test]
    fn resource_text_domains_are_resolved_only_in_the_comparison_view() -> Result<(), &'static str>
    {
        let row = json!({"guid": sourced(&json!("guid")), "name": sourced(&json!("name:4:@key")),
            "description": sourced(&json!("text:4:@key")), "groupGuids": sourced(&json!([]))});
        let text = |id, translation| {
            json!({"id": sourced(&json!(id)), "key": sourced(&json!("@key")),
            "translations": sourced(&json!({"en": sourced(&json!(translation))}))})
        };
        let actual = json!({"schemaVersion": 3,
            "definitions": {"resourceTypes": {"availability": "available", "rows": [row]},
                "commodities": {"availability": "available", "rows": [{"guid": sourced(&json!("commodity"))}]}},
            "projections": {"commodityPresentation": {"availability": "available", "rows": [{"commodityGuid": sourced(&json!("commodity"))}]}},
            "texts": {"publicNames": [text("name:4:@key", "Public")], "gameTexts": [text("text:4:@key", "Protected")]},
            "provenance": {"sources": [], "uses": []}});
        let comparison = project(actual.clone())?;
        assert_eq!(
            comparison["resourceTypes"][0]["name"],
            json!({"key":"@key", "en":"Public", "de":null})
        );
        assert_eq!(
            comparison["resourceTypes"][0]["description"]["en"],
            "Protected"
        );
        assert!(
            comparison.get("provenance").is_none(),
            "the semantic view excludes physical evidence"
        );
        assert_eq!(
            comparison["definitions"]["commodities"],
            actual["definitions"]["commodities"]
        );
        assert_eq!(comparison["projections"], actual["projections"]);
        assert_eq!(comparison["texts"], actual["texts"]);
        assert!(comparison.pointer("/definitions/resourceTypes").is_none());
        assert!(
            actual.get("resourceTypes").is_none(),
            "the actual product artifact keeps only the canonical table"
        );
        assert!(
            actual.get("provenance").is_some(),
            "the product artifact retains its evidence"
        );
        Ok(())
    }
}
