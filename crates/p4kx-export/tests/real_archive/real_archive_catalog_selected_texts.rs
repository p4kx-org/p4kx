// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashSet};
use std::fmt;

use p4kx_extraction::SourceInventory;
use serde::Serialize;
use serde::de::{DeserializeSeed, Deserializer, Error as _, SeqAccess, Visitor};
use serde_json::Value;

use super::compare_rows::Projection;
use super::{Origins, TestResult};
use crate::catalog_capture::CatalogCapture;

const PUBLIC: &str = "/texts/publicNames";
const PROTECTED: &str = "/texts/gameTexts";

#[derive(Default)]
pub(crate) struct ScopedTextProof {
    public: Texts,
    protected: Texts,
}

type Texts = BTreeMap<String, Vec<Value>>;

impl ScopedTextProof {
    pub(crate) fn retain(
        &mut self,
        public: &[impl Serialize],
        protected: &[impl Serialize],
        inventory: &SourceInventory,
        origins: &mut Origins,
    ) -> TestResult {
        self.retain_with(public, protected, |wires| origins.extend(inventory, wires))
    }

    pub(super) fn retain_with(
        &mut self,
        public: &[impl Serialize],
        protected: &[impl Serialize],
        mut capture: impl FnMut(&[Value]) -> TestResult,
    ) -> TestResult {
        retain(&mut self.public, public, &mut capture)?;
        retain(&mut self.protected, protected, &mut capture)
    }

    pub(super) fn project<'scope>(&'scope self, capture: &mut CatalogCapture<Projection<'scope>>) {
        for (path, texts) in self.domains() {
            if !texts.is_empty() {
                capture.project(path, Projection::ScopedTexts(SelectedTexts(texts)));
            }
        }
    }

    pub(super) fn verify(&self, catalog: &Value) -> TestResult {
        for (path, texts) in self.domains() {
            if !texts.is_empty() && catalog.pointer(path) != Some(&Value::from(texts.len())) {
                return Err(
                    format!("{path}: the retained text scope is missing or incomplete").into(),
                );
            }
        }
        Ok(())
    }

    const fn domains(&self) -> [(&'static str, &Texts); 2] {
        [(PUBLIC, &self.public), (PROTECTED, &self.protected)]
    }
}

fn retain(
    expected: &mut Texts,
    rows: &[impl Serialize],
    capture: &mut impl FnMut(&[Value]) -> TestResult,
) -> TestResult {
    if rows.is_empty() {
        return Ok(());
    }
    let wires = rows
        .iter()
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    capture(&wires)?;
    let mut seen = HashSet::new();
    for value in wires {
        let id = text_identity(&value).ok_or("the SDK omitted its selected text ID")?;
        if !seen.insert(id.to_owned()) {
            return Err("the SDK repeated a selected text ID within one query".into());
        }
        expected.entry(id.to_owned()).or_default().push(value);
    }
    Ok(())
}

fn text_identity(row: &Value) -> Option<&str> {
    if row.pointer("/id/state").and_then(Value::as_str) == Some("present") {
        row.pointer("/id/value").and_then(Value::as_str)
    } else {
        None
    }
}

#[derive(Clone, Copy)]
pub(super) struct SelectedTexts<'scope>(&'scope Texts);

impl<'de> DeserializeSeed<'de> for SelectedTexts<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for SelectedTexts<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a canonical selected text row array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        let mut seen = HashSet::new();
        while let Some(row) = sequence.next_element::<Value>()? {
            let id =
                text_identity(&row).ok_or_else(|| A::Error::custom("missing retained text ID"))?;
            let Some((id, expected)) = self.0.get_key_value(id) else {
                continue;
            };
            if !seen.insert(id) {
                return Err(A::Error::custom("the retained text repeats a selected ID"));
            }
            if expected.iter().any(|value| !matches_scope(value, &row)) {
                return Err(A::Error::custom(
                    "retained selected text payload or selected field contributors differ",
                ));
            }
        }
        if seen.len() != self.0.len() {
            return Err(A::Error::custom(
                "missing retained text for a selected SDK reference",
            ));
        }
        Ok(Value::from(seen.len()))
    }
}

fn matches_scope(expected: &Value, retained: &Value) -> bool {
    match (expected, retained) {
        (Value::Object(expected), Value::Object(retained)) => {
            expected.len() == retained.len()
                && expected.iter().all(|(field, value)| {
                    retained.get(field).is_some_and(|other| {
                        if field == "sourceUsesRefs" {
                            contains_uses(value, other)
                        } else {
                            matches_scope(value, other)
                        }
                    })
                })
        }
        (Value::Array(expected), Value::Array(retained)) => {
            expected.len() == retained.len()
                && expected
                    .iter()
                    .zip(retained)
                    .all(|(value, other)| matches_scope(value, other))
        }
        _ => expected == retained,
    }
}

fn contains_uses(expected: &Value, retained: &Value) -> bool {
    let (Some(expected), Some(retained)) = (expected.as_array(), retained.as_array()) else {
        return false;
    };
    retained
        .iter()
        .filter(|value| expected.contains(value))
        .eq(expected)
}

#[test]
fn selected_text_comparison_preserves_payload_and_local_contributors() -> TestResult {
    use serde_json::json;

    let text = |id, payload, uses| {
        json!({
            "id": {"state": "present", "value": id},
            "translations": {"value": payload, "sourceUsesRefs": uses},
        })
    };
    let expected = text("selected", "same translation", vec!["owner", "locale"]);
    let rows = Texts::from_iter([("selected".to_owned(), vec![expected])]);
    let mut capture = CatalogCapture::default();
    capture.project(PROTECTED, Projection::ScopedTexts(SelectedTexts(&rows)));
    let read = |texts| {
        let wire = serde_json::to_vec(&json!({"texts": {"gameTexts": texts}}))?;
        capture.read(wire.as_slice())
    };
    read(vec![
        text("irrelevant", "unrelated", vec![]),
        text(
            "selected",
            "same translation",
            vec!["other-owner", "owner", "locale"],
        ),
    ])?;
    assert!(
        read(vec![text("selected", "changed", vec!["owner", "locale"])]).is_err(),
        "selected text payload must agree"
    );
    assert!(
        read(vec![text(
            "selected",
            "same translation",
            vec!["owner", "other"]
        )])
        .is_err(),
        "an unrelated use cannot replace the selected contributor"
    );
    assert!(
        read(vec![text(
            "selected",
            "same translation",
            vec!["locale", "owner"]
        )])
        .is_err(),
        "the selected contributor sequence must be preserved"
    );
    assert!(read(vec![]).is_err(), "missing selected text must fail");
    Ok(())
}
