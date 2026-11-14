// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{StructValue, Value as Source, ValueError};
use serde_json::{Value, json};

use super::TestResult;
use super::original::{self, Locations, Selected};
use super::provenance::Origins;
use crate::catalog_v3_consumer;
use crate::real_archive_contract_templates::mission_graph::{self as graph, Mission};

const KIND: &str = "autoSpawns";

pub(super) fn compare<'source>(
    mission: &Mission<'_>,
    selected: &[Selected<'_, 'source>],
    locations: &Locations<'source>,
    origins: &Origins<'_>,
    owner: u32,
) -> TestResult {
    let checked = Checked {
        mission,
        locations,
        origins,
        owner,
    };
    let mut expected = HashSet::new();
    let properties = graph::array(mission.projection, "properties")?;
    let meanings = graph::array(mission.projection, "valueMeanings")?;
    for property in selected {
        let property_link = link(locations.ordinal(property.property)?);
        let expected_value = link(locations.ordinal(property.value)?);
        let objective = property
            .objective
            .map(|value| locations.ordinal(value).map(link))
            .transpose()?
            .unwrap_or(Value::Null);
        let actual = unique(properties, &property_link)?;
        assert!(
            expected.insert(locations.ordinal(property.property)?)
                && actual.get("objective") == Some(&objective)
                && actual.get("value") == Some(&expected_value)
                && actual.get("valueKind").and_then(Value::as_str) == Some(KIND),
            "the selected AutoSpawns property keeps its physical value and objective links"
        );
        let meaning = unique(meanings, &expected_value)?;
        let kind = meaning.get("kind").ok_or("sourced AutoSpawns kind")?;
        assert!(
            meaning.get("status").and_then(Value::as_str) == Some("recognized")
                && catalog_v3_consumer::present(kind)?.as_str() == Some(KIND),
            "the selected source has a recognized AutoSpawns interpretation"
        );
        checked.origin(kind, property.value, "$type")?;
        checked.auto_spawns(property.value)?;
    }
    assert_eq!(
        properties
            .iter()
            .filter(|property| property.get("valueKind").and_then(Value::as_str) == Some(KIND))
            .count(),
        expected.len(),
        "the owner projects exactly its original selected AutoSpawns properties"
    );
    Ok(())
}

struct Checked<'view, 'catalog, 'source> {
    mission: &'view Mission<'catalog>,
    locations: &'view Locations<'source>,
    origins: &'view Origins<'catalog>,
    owner: u32,
}

impl<'source> Checked<'_, '_, 'source> {
    fn auto_spawns(&self, source: &Source<'source>) -> TestResult {
        let actual = self.node(source)?;
        let authored = self.structure(source, actual, original::AUTO_SPAWNS)?;
        self.member(authored, actual, "autoSpawns", Self::reference)?;
        self.member(authored, actual, "searchOptions", Self::search)
    }

    fn search(&self, source: &Source<'source>, actual: &Value) -> TestResult {
        let Some(options) = original::structure(source) else {
            return self.null(source, actual);
        };
        let name = options.type_name();
        let options = self.structure(source, actual, name)?;
        match name {
            "SpawnClosetSearchOptionsBase" => Ok(()),
            "SpawnClosetSearchOptions_EntityDeclaration" => {
                self.member(options, actual, "entityDeclaration", Self::reference)
            }
            "SpawnClosetSearchOptions_Tags" => {
                self.member(options, actual, "closetPositiveTags", Self::tags)?;
                self.member(options, actual, "closetNegativeTags", Self::tags)
            }
            _ => Err("unsupported original closet-search type".into()),
        }
    }

    fn tags(&self, source: &Source<'source>, actual: &Value) -> TestResult {
        if matches!(source, Source::Null) {
            return self.null(source, actual);
        }
        let list = self.structure(source, actual, "TagList")?;
        self.member(list, actual, "tags", Self::tag_array)
    }

    fn tag_array(&self, source: &Source<'source>, actual: &Value) -> TestResult {
        let Source::Array(items) = source else {
            return self.null(source, actual);
        };
        let value = payload(actual, "array")?;
        self.origin(cell(actual)?, source, "$value")?;
        let emitted = graph::array(value, "elements")?;
        assert_eq!(emitted.len(), items.len(), "exact authored tag count");
        for (link, item) in emitted.iter().zip(items) {
            self.check_link(link, item)?;
            self.reference(item, self.mission.follow(link)?)?;
        }
        Ok(())
    }

    fn reference(&self, source: &Source<'source>, actual: &Value) -> TestResult {
        match source {
            Source::Null => self.null(source, actual),
            Source::Reference(target) if target.location().is_none() => {
                self.origin(cell(actual)?, source, "$value")?;
                let target_row = payload(actual, "recordReference")?
                    .get("value")
                    .ok_or("original Record Reference fields")?;
                assert_eq!(
                    target_row.get("status").and_then(Value::as_str),
                    Some("present"),
                    "the original Reference resolves to an actual Record"
                );
                for (field, expected, header) in [
                    ("guid", json!(target.guid().to_string()), None),
                    (
                        "targetRecordIndex",
                        json!(target.record_index()),
                        Some("recordIndex"),
                    ),
                    ("recordPath", json!(target.record_path()), Some("fileName")),
                    ("targetType", json!(target.type_name()), Some("sourceType")),
                ] {
                    let value = target_row.get(field).ok_or("source-backed target field")?;
                    assert!(
                        catalog_v3_consumer::present(value)? == &expected,
                        "the physical reference target retains its exact {field}"
                    );
                    self.origins.reference_field(
                        value,
                        self.owner,
                        self.locations.ordinal(source)?,
                        target.record_index(),
                        header,
                    )?;
                }
                Ok(())
            }
            Source::Error(ValueError::DanglingReference { guid }) => {
                Self::unresolved(actual, "danglingReference", &guid.to_string())?;
                let value = actual
                    .pointer("/originalError/guid")
                    .ok_or("dangling GUID")?;
                self.origin(value, source, "$error.guid")
            }
            Source::Error(ValueError::UnexpectedReferenceType { guid, .. }) => {
                Self::unresolved(actual, "unexpectedReferenceType", &guid.to_string())
            }
            _ => Err("original field is not a nullable Record Reference".into()),
        }
    }

    fn unresolved(actual: &Value, error: &str, guid: &str) -> TestResult {
        let value = cell(actual)?;
        assert!(
            catalog_v3_consumer::source_value(value)?.is_none()
                && value.get("state").and_then(Value::as_str) == Some("unreadable")
                && actual
                    .pointer("/originalError/type")
                    .and_then(Value::as_str)
                    == Some(error)
                && actual
                    .pointer("/originalError/guid/value")
                    .and_then(Value::as_str)
                    == Some(guid),
            "the original failed target keeps its exact classification and retained GUID"
        );
        Ok(())
    }

    fn structure<'source_value>(
        &self,
        source: &'source_value Source<'source>,
        actual: &Value,
        name: &str,
    ) -> TestResult<&'source_value StructValue<'source>> {
        let authored = original::structure(source).ok_or("original authored Struct")?;
        let value = payload(actual, "struct")?;
        let source_type = value.get("sourceType").ok_or("sourced concrete type")?;
        assert!(
            authored.type_name() == name
                && catalog_v3_consumer::present(source_type)?.as_str() == Some(name)
                && graph::array(value, "fields")?.len() == authored.fields().len(),
            "the actual concrete Struct and its original field population survive"
        );
        self.origin(cell(actual)?, source, "$value")?;
        self.origin(source_type, source, "$type")?;
        Ok(authored)
    }

    fn member(
        &self,
        source: &StructValue<'source>,
        actual: &Value,
        name: &str,
        check: impl FnOnce(&Self, &Source<'source>, &Value) -> TestResult,
    ) -> TestResult {
        let fields = graph::array(payload(actual, "struct")?, "fields")?;
        let Some((ordinal, value)) = source
            .fields()
            .iter()
            .enumerate()
            .find(|(_, field)| field.name() == name)
        else {
            assert!(
                self.mission.member(actual, name)?.is_none(),
                "an absent original member remains absent"
            );
            return Ok(());
        };
        let field = fields
            .get(ordinal)
            .ok_or("original named declaration slot")?;
        let field_name = field.get("name").ok_or("sourced original field name")?;
        assert!(
            catalog_v3_consumer::present(field_name)?.as_str() == Some(name),
            "the original named declaration remains in its own slot"
        );
        self.origin(field_name, value.value(), "$fieldName")?;
        let link = field.get("value").ok_or("original member link")?;
        self.check_link(link, value.value())?;
        check(self, value.value(), self.mission.follow(link)?)
    }

    fn null(&self, source: &Source<'source>, actual: &Value) -> TestResult {
        let value = cell(actual)?;
        assert!(
            matches!(source, Source::Null)
                && catalog_v3_consumer::source_value(value)?.is_none()
                && value.get("state").and_then(Value::as_str) == Some("null"),
            "an authored null remains distinct from absent or failed source"
        );
        self.origin(value, source, "$value")
    }

    fn check_link(&self, actual: &Value, source: &Source<'source>) -> TestResult {
        assert!(
            actual == &link(self.locations.ordinal(source)?),
            "owner-local link resolves to the exact original occurrence"
        );
        Ok(())
    }

    fn node(&self, source: &Source<'source>) -> TestResult<&Value> {
        Ok(self
            .mission
            .follow(&link(self.locations.ordinal(source)?))?)
    }

    fn origin(&self, cell: &Value, source: &Source<'source>, field: &str) -> TestResult {
        self.origins
            .occurrence(cell, self.owner, self.locations.ordinal(source)?, field)
    }
}

fn link(ordinal: usize) -> Value {
    json!({"valueIndex": ordinal.to_string()})
}

fn unique<'rows>(rows: &'rows [Value], source: &Value) -> TestResult<&'rows Value> {
    let mut matches = rows.iter().filter(|row| row.get("source") == Some(source));
    let row = matches.next().ok_or("exact selected occurrence")?;
    assert!(matches.next().is_none(), "one row per original occurrence");
    Ok(row)
}

fn cell(node: &Value) -> TestResult<&Value> {
    node.get("value")
        .ok_or_else(|| "original sourced datum".into())
}

fn payload<'node>(node: &'node Value, kind: &str) -> TestResult<&'node Value> {
    let value = catalog_v3_consumer::present(cell(node)?)?;
    assert_eq!(
        value.get("type").and_then(Value::as_str),
        Some(kind),
        "the retained original value has the corresponding type"
    );
    Ok(value)
}
