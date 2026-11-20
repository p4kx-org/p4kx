// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};

#[derive(Clone, Copy)]
pub(super) struct SelectedOrigins<'scope>(pub(super) &'scope Map<String, Value>);

impl<'de> DeserializeSeed<'de> for SelectedOrigins<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for SelectedOrigins<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Catalog provenance row array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Value, A::Error> {
        use serde::de::Error as _;
        let mut selected = Map::new();
        while let Some(row) = sequence.next_element_seed(SelectOrigin(self.0))? {
            if let Some((id, row)) = row
                && selected.insert(id, row).is_some()
            {
                return Err(A::Error::custom(
                    "duplicate selected Catalog provenance identifier",
                ));
            }
        }
        Ok(Value::Object(selected))
    }
}

#[derive(Clone, Copy)]
struct SelectOrigin<'scope>(&'scope Map<String, Value>);

impl<'de> DeserializeSeed<'de> for SelectOrigin<'_> {
    type Value = Option<(String, Value)>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for SelectOrigin<'_> {
    type Value = Option<(String, Value)>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Catalog provenance row with a string identifier")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut row: A) -> Result<Self::Value, A::Error> {
        use serde::de::Error as _;
        let mut identity = OriginId::Missing;
        let mut fields = Map::new();
        // Canonical rows write the identifier first.
        loop {
            let selection = if matches!(identity, OriginId::Unselected) {
                ReadField::Skip
            } else {
                ReadField::Keep
            };
            let Some(field) = row.next_key_seed(selection)? else {
                break;
            };
            match field {
                OriginField::Id => {
                    if !matches!(identity, OriginId::Missing) {
                        return Err(A::Error::duplicate_field("id"));
                    }
                    identity = row.next_value_seed(SelectId(self.0))?;
                    if matches!(identity, OriginId::Unselected) {
                        fields.clear();
                    }
                }
                OriginField::Kept(name) => {
                    if fields.insert(name, row.next_value::<Value>()?).is_some() {
                        return Err(A::Error::custom("duplicate Catalog provenance field"));
                    }
                }
                OriginField::Skipped => {
                    row.next_value::<IgnoredAny>()?;
                }
            }
        }
        match identity {
            OriginId::Missing => Err(A::Error::missing_field("id")),
            OriginId::Unselected => Ok(None),
            OriginId::Selected(id) => {
                fields.insert("id".to_owned(), Value::String(id.clone()));
                Ok(Some((id, Value::Object(fields))))
            }
        }
    }
}

enum ReadField {
    Keep,
    Skip,
}

enum OriginField {
    Id,
    Kept(String),
    Skipped,
}

impl<'de> DeserializeSeed<'de> for ReadField {
    type Value = OriginField;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<OriginField, D::Error> {
        deserializer.deserialize_identifier(self)
    }
}

impl Visitor<'_> for ReadField {
    type Value = OriginField;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Catalog provenance field name")
    }

    fn visit_str<E: serde::de::Error>(self, key: &str) -> Result<OriginField, E> {
        Ok(match (self, key) {
            (_, "id") => OriginField::Id,
            (Self::Keep, key) => OriginField::Kept(key.to_owned()),
            (Self::Skip, _) => OriginField::Skipped,
        })
    }
}

struct SelectId<'scope>(&'scope Map<String, Value>);

enum OriginId {
    Missing,
    Selected(String),
    Unselected,
}

impl<'de> DeserializeSeed<'de> for SelectId<'_> {
    type Value = OriginId;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<OriginId, D::Error> {
        deserializer.deserialize_str(self)
    }
}

impl Visitor<'_> for SelectId<'_> {
    type Value = OriginId;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a Catalog provenance identifier")
    }

    fn visit_str<E: serde::de::Error>(self, id: &str) -> Result<OriginId, E> {
        Ok(if self.0.contains_key(id) {
            OriginId::Selected(id.to_owned())
        } else {
            OriginId::Unselected
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::SelectedOrigins;
    use crate::catalog_capture::CatalogCapture;

    const USES: &str = "/provenance/uses";

    #[test]
    fn keeps_the_full_selected_row_regardless_of_field_order() -> serde_json::Result<()> {
        let expected = Map::from_iter([(
            "selected".to_owned(),
            json!({
                "location": {"indices": [1, 2]},
                "id": "selected",
                "role": "payload"
            }),
        )]);
        let mut capture = CatalogCapture::default();
        capture.project(USES, SelectedOrigins(&expected));
        let input = br#"{"provenance":{"uses":[
            {"id":"unused","payload":[{"id":"selected"}]},
            {"location":{"indices":[1,2]},"id":"selected","role":"payload"}
        ]}}"#;
        let selected = capture.read(input.as_slice())?;
        assert_eq!(
            selected.pointer(USES).and_then(Value::as_object),
            Some(&expected),
            "selection retains fields on either side of the row identifier"
        );
        Ok(())
    }

    #[test]
    fn rejects_ambiguous_or_missing_row_identifiers() {
        let expected = Map::from_iter([("selected".to_owned(), Value::Null)]);
        let mut capture = CatalogCapture::default();
        capture.project(USES, SelectedOrigins(&expected));
        for (rows, reason) in [
            (
                r#"[{"id":"selected"},{"id":"selected"}]"#,
                "duplicate selected Catalog provenance identifier",
            ),
            (
                r#"[{"id":"unused","value":12,"id":"selected"}]"#,
                "duplicate field `id`",
            ),
            (
                r#"[{"id":"selected","id":"unused"}]"#,
                "duplicate field `id`",
            ),
            (r#"[{"value":12}]"#, "missing field `id`"),
            (r#"[{"id":42}]"#, "expected a Catalog provenance identifier"),
        ] {
            let document = format!(r#"{{"provenance":{{"uses":{rows}}}}}"#);
            let failure = capture
                .read(document.as_bytes())
                .expect_err("an ambiguous provenance row cannot establish parity");
            assert!(failure.to_string().contains(reason), "{failure}");
        }
    }
}
