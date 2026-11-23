// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::Deserialize;
use serde::de::{DeserializeSeed, Deserializer, Error, IgnoredAny, SeqAccess, Visitor};
use serde_json::Value;

use super::selected_origins::SelectedOrigins;
use crate::real_archive_catalog_size::replay::bindings::check_catalog_selection;

#[derive(Clone, Copy)]
pub(super) enum Projection<'scope> {
    CompleteSelection,
    Rows(ComparedRows<'scope>),
    Origins(SelectedOrigins<'scope>),
}

impl<'de> DeserializeSeed<'de> for Projection<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        match self {
            Self::CompleteSelection => {
                let value = Value::deserialize(deserializer)?;
                check_catalog_selection(&value).map_err(D::Error::custom)?;
                Ok(value)
            }
            Self::Rows(rows) => rows.deserialize(deserializer),
            Self::Origins(origins) => origins.deserialize(deserializer),
        }
    }
}

#[derive(Clone, Copy)]
enum RowAddress<'scope> {
    Table(&'scope str),
    BareArray(&'scope str),
}

impl fmt::Display for RowAddress<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Table(path) => write!(formatter, "{path}/rows"),
            Self::BareArray(path) => formatter.write_str(path),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ComparedRows<'scope> {
    address: RowAddress<'scope>,
    expected: &'scope [Value],
}

impl<'scope> ComparedRows<'scope> {
    pub(super) const fn new(dataset: &'scope str, expected: &'scope [Value]) -> Self {
        Self {
            address: RowAddress::Table(dataset),
            expected,
        }
    }

    pub(super) const fn bare_array(dataset: &'scope str, expected: &'scope [Value]) -> Self {
        Self {
            address: RowAddress::BareArray(dataset),
            expected,
        }
    }
}

impl<'de> DeserializeSeed<'de> for ComparedRows<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for ComparedRows<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a complete ordered Catalog row array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        for (index, expected) in self.expected.iter().enumerate() {
            let row = sequence.next_element::<Value>()?.ok_or_else(|| {
                A::Error::custom(format!("missing compared row at {}/{index}", self.address))
            })?;
            if &row != expected {
                return Err(A::Error::custom(format!(
                    "Catalog and SDK payload or provenance differ at {}/{index}",
                    self.address,
                )));
            }
        }
        if sequence.next_element::<IgnoredAny>()?.is_some() {
            return Err(A::Error::custom(format!(
                "extra compared row at {}/{}",
                self.address,
                self.expected.len()
            )));
        }
        Ok(Value::from(self.expected.len()))
    }
}

#[test]
fn row_stream_reports_only_counts_and_structural_mismatches() -> super::TestResult {
    use serde_json::json;

    let expected = [json!({"amount": 3}), json!({"amount": 7})];
    let seed = ComparedRows::new("/synthetic", &expected);
    let accepted = r#"[{"amount":3},{"amount":7}]"#;
    let mut decoder = serde_json::Deserializer::from_str(accepted);
    assert_eq!(
        seed.deserialize(&mut decoder)?,
        json!(2),
        "complete input yields its verified row count"
    );
    decoder.end()?;
    for invalid in [
        r#"[{"amount":3}]"#,
        r#"[{"amount":7},{"amount":3}]"#,
        r#"[{"amount":3},{"amount":7},{"amount":8}]"#,
        r#"[{"amount":3},{"amount":19}]"#,
    ] {
        let mut decoder = serde_json::Deserializer::from_str(invalid);
        assert!(
            seed.deserialize(&mut decoder).is_err(),
            "shape, count and exact original row order are all checked"
        );
    }
    Ok(())
}

#[test]
fn bare_array_capture_requires_every_ordered_row_at_the_actual_member() -> super::TestResult {
    use serde_json::json;

    use super::check_row_array_count;
    use crate::catalog_capture::CatalogCapture;

    const PATH: &str = "/shipStoreSlugs";
    let expected = [
        json!({"slug": "a", "retired": false}),
        json!({"slug": "b", "retired": true}),
    ];
    let mut capture = CatalogCapture::default();
    capture.project(
        PATH,
        Projection::Rows(ComparedRows::bare_array(PATH, &expected)),
    );

    let value = capture.read(
        br#"{"shipStoreSlugs":[{"slug":"a","retired":false},{"slug":"b","retired":true}]}"#
            .as_slice(),
    )?;
    check_row_array_count(&value, PATH, expected.len())?;

    let missing_member = capture.read(br#"{"other":[]}"#.as_slice())?;
    assert!(
        check_row_array_count(&missing_member, PATH, expected.len()).is_err(),
        "an absent array cannot be reported as an exact match"
    );
    for (source, position) in [
        (
            r#"{"shipStoreSlugs":[{"slug":"b","retired":true},{"slug":"a","retired":false}]}"#,
            "/shipStoreSlugs/0",
        ),
        (
            r#"{"shipStoreSlugs":[{"slug":"a","retired":false}]}"#,
            "/shipStoreSlugs/1",
        ),
        (
            r#"{"shipStoreSlugs":[{"slug":"a","retired":false},{"slug":"b","retired":true},{"slug":"c","retired":true}]}"#,
            "/shipStoreSlugs/2",
        ),
        (
            r#"{"shipStoreSlugs":[{"slug":"a","retired":true},{"slug":"b","retired":true}]}"#,
            "/shipStoreSlugs/0",
        ),
    ] {
        let error = capture
            .read(source.as_bytes())
            .expect_err("an incomplete or different array");
        assert!(
            error.to_string().contains(position),
            "the rejection reports the structural bare-array position"
        );
    }
    assert!(
        capture
            .read(br#"{"shipStoreSlugs":{"rows":[]}}"#.as_slice())
            .is_err(),
        "a wrapped table cannot substitute for the bare array"
    );
    Ok(())
}

#[test]
fn incomplete_catalog_header_fails_before_the_bulk_payload() -> super::TestResult {
    use p4kx_export::Kind;
    use serde_json::{Map, json};

    use crate::catalog_capture::CatalogCapture;
    use crate::real_archive_catalog_size::replay::bindings::catalog_selection_names;

    let omitted = Kind::SubsumptionEvents.name();
    let completeness: Map<String, Value> = catalog_selection_names()
        .filter(|name| *name != omitted)
        .map(|name| (name.to_owned(), json!({"status": "complete"})))
        .collect();
    let header = serde_json::to_string(&completeness)?;
    let input = format!(r#"{{"completeness":{header},"definitions":@}}"#);
    let mut capture = CatalogCapture::default();
    capture.project("/completeness", Projection::CompleteSelection);
    capture.keep("/definitions");
    let error = capture
        .read(input.as_bytes())
        .expect_err("ineligible header must fail before decoding the bulk payload");
    assert!(
        error.to_string().contains(&format!("missing: {omitted})")),
        "{error}"
    );
    Ok(())
}
