// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::io::Read;
use std::path::Path;

use serde::Deserializer as _;
use serde::de::{IgnoredAny, MapAccess, Visitor};
use serde_json::Value;

use crate::real_archive_catalog_size::artifact;

use crate::real_archive_catalog_size::replay::bindings::check_catalog_selection;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(crate) fn check(directory: &Path) -> TestResult {
    let selection = artifact::read_completed_prefix(directory, |input| {
        Ok(read_member(input, "completeness")?)
    })?;
    if let Some(selection) = &selection {
        check_catalog_selection(selection)?;
    }
    Ok(())
}

pub(crate) fn source(directory: &Path) -> TestResult<Value> {
    artifact::read_completed_prefix(directory, |input| {
        read_member(input, "source")?.ok_or_else(|| "missing leading Catalog source".into())
    })
}

fn read_member(input: impl Read, field: &'static str) -> serde_json::Result<Option<Value>> {
    let mut prefix = Prefix::Reading;
    let mut decoder = serde_json::Deserializer::from_reader(input);
    let result = decoder.deserialize_map(MetadataPrefix {
        field,
        found: &mut prefix,
    });
    match prefix {
        Prefix::Found(value) => Ok(Some(value)),
        Prefix::Payload => Ok(None),
        Prefix::Reading => result.map(|()| None),
    }
}

enum Prefix {
    Reading,
    Found(Value),
    Payload,
}

struct MetadataPrefix<'a> {
    field: &'static str,
    found: &'a mut Prefix,
}

impl<'de> Visitor<'de> for MetadataPrefix<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Catalog root object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                name if name == self.field => {
                    *self.found = Prefix::Found(map.next_value()?);
                    return Err(serde::de::Error::custom("Catalog metadata member read"));
                }
                "schemaVersion" | "source" | "exportHealth" | "completeness" => {
                    map.next_value::<IgnoredAny>()?;
                }
                _ => {
                    *self.found = Prefix::Payload;
                    return Err(serde::de::Error::custom("Catalog payload starts"));
                }
            }
        }
        Ok(())
    }
}

#[test]
fn missing_required_kind_is_reported_from_the_prefix() -> TestResult {
    use p4kx_export::Kind;
    use serde_json::json;

    use crate::real_archive_catalog_size::replay::bindings::catalog_selection_names;

    let required = Kind::SubsumptionEvents.name();
    let selection = Value::Object(
        catalog_selection_names()
            .filter(|name| *name != required)
            .map(|name| (name.to_owned(), json!({"status": "complete"})))
            .collect(),
    );
    let prefix = format!(
        r#"{{"schemaVersion":3,"source":{{}},"completeness":{selection},"items":<unparsed>"#,
    );
    let selection =
        read_member(prefix.as_bytes(), "completeness")?.ok_or("present selection prefix")?;
    let Err(error) = check_catalog_selection(&selection) else {
        return Err("a retained selection missing a current Kind must fail".into());
    };
    assert!(
        error
            .to_string()
            .ends_with(&format!("(missing: {required})")),
        "only the missing current Kind is named in the preflight diagnostic"
    );
    Ok(())
}

#[test]
fn prefix_stops_before_a_payload_that_precedes_completeness() -> TestResult {
    let prefix = br#"{"schemaVersion":3,"items":<unparsed>"#;
    assert!(
        read_member(prefix.as_slice(), "completeness")?.is_none(),
        "non-leading completeness is left for full Catalog validation"
    );
    Ok(())
}
