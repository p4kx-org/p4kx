// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::Deserialize;
use serde::de::{DeserializeSeed, Deserializer, Error, IgnoredAny, SeqAccess, Visitor};
use serde_json::Value;

use super::digest_parity::{DigestedOrigins, DigestedRows};
use super::selected_origins::SelectedOrigins;
use super::selected_ships::SelectedShipIdentities;
use super::selected_texts::SelectedTexts;
use super::stream_parity::{MissionDispositionRows, RowDigest, StreamOrigins};
use crate::real_archive_catalog_size::replay::bindings::check_catalog_selection;

#[derive(Clone, Copy)]
pub(super) enum Projection<'scope> {
    CompleteSelection,
    ShipIdentities(SelectedShipIdentities),
    Rows(ComparedRows<'scope>),
    Origins(SelectedOrigins<'scope>),
    ScopedTexts(SelectedTexts<'scope>),
    DigestedRows(DigestedRows<'scope>),
    DigestedOrigins(DigestedOrigins<'scope>),
    StreamRows(RowDigest),
    StreamOrigins(StreamOrigins<'scope>),
    MissionDispositions(MissionDispositionRows),
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
            Self::ShipIdentities(ships) => ships.deserialize(deserializer),
            Self::Rows(rows) => rows.deserialize(deserializer),
            Self::Origins(origins) => origins.deserialize(deserializer),
            Self::ScopedTexts(texts) => texts.deserialize(deserializer),
            Self::DigestedRows(rows) => rows.deserialize(deserializer),
            Self::DigestedOrigins(origins) => origins.deserialize(deserializer),
            Self::StreamRows(rows) => rows.deserialize(deserializer),
            Self::StreamOrigins(origins) => origins.deserialize(deserializer),
            Self::MissionDispositions(kinds) => kinds.deserialize(deserializer),
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
pub(super) enum RowSelection {
    All,
    PhysicalShip(u32),
}

impl RowSelection {
    fn includes<E: Error>(self, row: &Value) -> Result<bool, E> {
        let Self::PhysicalShip(ship) = self else {
            return Ok(true);
        };
        let index = row
            .pointer("/shipRecordIndex/value")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| E::custom("a Catalog flight row has no physical Ship Record index"))?;
        if row
            .pointer("/shipRecordIndex/state")
            .and_then(Value::as_str)
            != Some("present")
        {
            return Err(E::custom(
                "a Catalog flight row has no present Ship identity",
            ));
        }
        Ok(index == ship)
    }
}

#[derive(Clone, Copy)]
pub(super) struct ComparedRows<'scope> {
    address: RowAddress<'scope>,
    expected: &'scope [Value],
    selection: RowSelection,
}

impl<'scope> ComparedRows<'scope> {
    pub(super) const fn table(
        dataset: &'scope str,
        expected: &'scope [Value],
        selection: RowSelection,
    ) -> Self {
        Self {
            address: RowAddress::Table(dataset),
            expected,
            selection,
        }
    }

    pub(super) const fn bare_array(dataset: &'scope str, expected: &'scope [Value]) -> Self {
        Self {
            address: RowAddress::BareArray(dataset),
            expected,
            selection: RowSelection::All,
        }
    }

    fn next_matching_row<'de, A: SeqAccess<'de>>(
        &self,
        sequence: &mut A,
    ) -> Result<Option<Value>, A::Error> {
        while let Some(row) = sequence.next_element::<Value>()? {
            if self.selection.includes::<A::Error>(&row)? {
                return Ok(Some(row));
            }
        }
        Ok(None)
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
            let row = self.next_matching_row(&mut sequence)?.ok_or_else(|| {
                A::Error::custom(format!("missing compared row at {}/{index}", self.address))
            })?;
            if let Some(path) = first_difference(expected, &row) {
                return Err(A::Error::custom(format!(
                    "Catalog and SDK payload or provenance differ at {}/{index}{path}",
                    self.address,
                )));
            }
        }
        let has_extra = match self.selection {
            RowSelection::All => sequence.next_element::<IgnoredAny>()?.is_some(),
            RowSelection::PhysicalShip(_) => self.next_matching_row(&mut sequence)?.is_some(),
        };
        if has_extra {
            return Err(A::Error::custom(format!(
                "extra compared row at {}/{}",
                self.address,
                self.expected.len()
            )));
        }
        Ok(Value::from(self.expected.len()))
    }
}

pub(crate) fn first_difference(expected: &Value, actual: &Value) -> Option<String> {
    if expected == actual {
        return None;
    }
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => {
            for (key, child) in expected {
                match actual.get(key) {
                    Some(actual) => {
                        if let Some(path) = first_difference(child, actual) {
                            return Some(format!("/{key}{path}"));
                        }
                    }
                    None => return Some(format!("/{key}")),
                }
            }
            Some("/additionalKey".into())
        }
        (Value::Array(expected), Value::Array(actual)) if expected.len() == actual.len() => {
            expected
                .iter()
                .zip(actual)
                .enumerate()
                .find_map(|(at, (left, right))| {
                    first_difference(left, right).map(|path| format!("/{at}{path}"))
                })
        }
        _ => Some(String::new()),
    }
}

#[test]
fn row_stream_reports_only_counts_and_structural_mismatches() -> super::TestResult {
    use serde_json::json;

    let expected = [json!({"amount": 3}), json!({"amount": 7})];
    let seed = ComparedRows::table("/synthetic", &expected, RowSelection::All);
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
fn mixed_table_capture_preserves_selected_physical_ship_rows() -> super::TestResult {
    use serde_json::json;

    use crate::catalog_capture::CatalogCapture;

    let row = |ship, part| {
        json!({
            "shipRecordIndex": {"state": "present", "value": ship},
            "shipGuid": "same-nominal-guid",
            "part": part,
        })
    };
    let first = row(10, "first");
    let second = row(10, "second");
    let unrelated = row(20, "other physical owner");
    let expected = [first.clone(), second.clone()];
    let whole = [json!({"kind": "armor"})];
    let selected = ComparedRows::table("/flight", &expected, RowSelection::PhysicalShip(10));

    let mut capture = CatalogCapture::default();
    capture.project("/flight/rows", Projection::Rows(selected));
    capture.project(
        "/armor/rows",
        Projection::Rows(ComparedRows::table("/armor", &whole, RowSelection::All)),
    );
    let input = json!({
        "flight": {"rows": [unrelated, first, unrelated, second]},
        "armor": {"rows": whole},
    })
    .to_string();
    let captured = capture.read(input.as_bytes())?;
    assert_eq!(captured.pointer("/flight/rows"), Some(&json!(2)));
    assert_eq!(captured.pointer("/armor/rows"), Some(&json!(1)));

    for (invalid, reason) in [
        (
            json!([first, unrelated, second, first]),
            "extra compared row",
        ),
        (json!([unrelated, first, unrelated]), "missing compared row"),
        (json!([second, unrelated, first]), "/flight/rows/0/part"),
        (
            json!([first, second, {"shipRecordIndex": {"state": "present", "value": "20"}}]),
            "no physical Ship",
        ),
        (
            json!([first, second, {"shipRecordIndex": {"state": "unavailable", "value": 20}}]),
            "no present Ship identity",
        ),
    ] {
        let input = invalid.to_string();
        let error = selected
            .deserialize(&mut serde_json::Deserializer::from_str(&input))
            .expect_err("an invalid physical row sequence must be refused");
        assert!(error.to_string().contains(reason), "{reason}: {error}");
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
