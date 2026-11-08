// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::Guid;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Failure, Result};

pub(super) const MAX_ROWS: usize = 16_384;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Counts {
    rows: usize,
    unique_guids: usize,
    distinct_subtypes_including_null: usize,
    null_subtypes: usize,
    shared_subtype_groups: usize,
    rows_in_shared_subtypes: usize,
}

pub(super) fn check(catalog: &Value) -> Result<Counts> {
    let definitions = rows(catalog, "/definitions/commodities")?;
    let presentation = rows(catalog, "/projections/commodityPresentation")?;
    let guids = identities(definitions, "guid", "duplicate-commodity-guid")?;
    let presented = identities(
        presentation,
        "commodityGuid",
        "duplicate-commodity-presentation-guid",
    )?;
    if guids != presented {
        return Err(Failure::check("commodity-presentation-identity-closure"));
    }
    let mut subtypes = BTreeMap::<Option<Guid>, usize>::new();
    for row in presentation {
        let subtype = match present(row, "subtypeGuid")? {
            Value::Null => None,
            value => Some(guid(value)?),
        };
        *subtypes.entry(subtype).or_default() += 1;
    }
    let shared: Vec<_> = subtypes
        .iter()
        .filter(|(guid, count)| guid.is_some() && **count > 1)
        .collect();
    Ok(Counts {
        rows: definitions.len(),
        unique_guids: guids.len(),
        distinct_subtypes_including_null: subtypes.len(),
        null_subtypes: subtypes.get(&None).copied().unwrap_or_default(),
        shared_subtype_groups: shared.len(),
        rows_in_shared_subtypes: shared.iter().map(|(_, count)| **count).sum(),
    })
}

fn rows<'catalog>(catalog: &'catalog Value, path: &str) -> Result<&'catalog [Value]> {
    let dataset = catalog
        .pointer(path)
        .ok_or_else(|| Failure::check("commodity-dataset-missing"))?;
    if dataset.get("availability").and_then(Value::as_str) != Some("available") {
        return Err(Failure::check("commodity-dataset-unavailable"));
    }
    let rows = dataset
        .get("rows")
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("commodity-dataset-rows"))?;
    if rows.is_empty() || rows.len() > MAX_ROWS {
        return Err(Failure::check("commodity-row-budget"));
    }
    Ok(rows)
}

fn identities(
    rows: &[Value],
    field: &str,
    duplicate_stage: &'static str,
) -> Result<BTreeSet<Guid>> {
    let mut guids = BTreeSet::new();
    for row in rows {
        if !guids.insert(guid(present(row, field)?)?) {
            return Err(Failure::check(duplicate_stage));
        }
    }
    Ok(guids)
}

fn present<'row>(row: &'row Value, field: &str) -> Result<&'row Value> {
    let carrier = row
        .get(field)
        .ok_or_else(|| Failure::check("commodity-sourced-field-missing"))?;
    if carrier.get("state").and_then(Value::as_str) != Some("present") {
        return Err(Failure::check("commodity-sourced-field-not-present"));
    }
    if carrier
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .is_none()
        || carrier
            .get("sourceSetComplete")
            .and_then(Value::as_bool)
            .is_none()
        || !matches!(
            carrier.get("containsEncryptedSource"),
            Some(Value::Null | Value::Bool(_))
        )
    {
        return Err(Failure::check("commodity-sourced-origin-shape"));
    }
    carrier
        .get("value")
        .ok_or_else(|| Failure::check("commodity-sourced-value-missing"))
}

fn guid(value: &Value) -> Result<Guid> {
    value
        .as_str()
        .ok_or_else(|| Failure::check("commodity-link-type"))?
        .parse()
        .map_err(|error| Failure::source("commodity-guid-format", error))
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::Guid;
    use serde_json::{Value, json};

    use super::{Counts, MAX_ROWS, Result, check};

    #[must_use]
    fn id(byte: u8) -> String {
        Guid::from_stored([byte; 16]).to_string()
    }

    #[must_use]
    fn sourced(value: Value) -> Value {
        Value::Object(
            [
                ("state".to_owned(), Value::String("present".to_owned())),
                ("value".to_owned(), value),
                ("sourceUsesRefs".to_owned(), Value::Array(Vec::new())),
                ("containsEncryptedSource".to_owned(), Value::Null),
                ("sourceSetComplete".to_owned(), Value::Bool(false)),
            ]
            .into_iter()
            .collect(),
        )
    }

    #[must_use]
    fn catalog() -> Value {
        let definitions: Vec<_> = (1..=4)
            .map(|byte| json!({"guid":sourced(json!(id(byte)))}))
            .collect();
        let presentation: Vec<_> = [(1, Some(9)), (2, Some(9)), (3, Some(8)), (4, None)]
            .into_iter()
            .map(|(byte, subtype)| {
                json!({
                    "commodityGuid":sourced(json!(id(byte))),
                    "subtypeGuid":sourced(json!(subtype.map(id)))
                })
            })
            .collect();
        json!({"schemaVersion":3,
            "definitions":{"commodities":{"availability":"available", "rows":definitions}},
            "projections":{"commodityPresentation":{"availability":"available", "rows":presentation}}})
    }

    #[test]
    fn primary_identity_and_shared_nullable_links_have_separate_counts() -> Result<()> {
        let mut catalog = catalog();
        catalog["commodities"] = json!([{"guid":id(1),"subtypeGuid":null}]);
        assert_eq!(
            check(&catalog)?,
            Counts {
                rows: 4,
                unique_guids: 4,
                distinct_subtypes_including_null: 3,
                null_subtypes: 1,
                shared_subtype_groups: 1,
                rows_in_shared_subtypes: 2
            },
            "only the canonical definition and presentation datasets supply counts"
        );
        Ok(())
    }

    #[test]
    fn duplicate_primary_and_presentation_guids_fail_independently() {
        for path in [
            "/definitions/commodities/rows/1/guid/value",
            "/projections/commodityPresentation/rows/1/commodityGuid/value",
        ] {
            let mut catalog = catalog();
            *catalog.pointer_mut(path).expect("synthetic identity") = json!(id(1));
            assert!(
                check(&catalog).is_err(),
                "each canonical identity must be unique"
            );
        }
    }

    #[test]
    fn mismatched_missing_and_extra_presentation_identities_fail() {
        let mut wrong = catalog();
        wrong["projections"]["commodityPresentation"]["rows"][0]["commodityGuid"] =
            sourced(json!(id(7)));
        assert!(
            check(&wrong).is_err(),
            "equal row counts do not prove identity closure"
        );
        for path in [
            "/definitions/commodities/rows",
            "/projections/commodityPresentation/rows",
        ] {
            let mut missing = catalog();
            let _ = missing
                .pointer_mut(path)
                .and_then(Value::as_array_mut)
                .expect("synthetic rows")
                .pop();
            assert!(
                check(&missing).is_err(),
                "each canonical family must represent exactly the same identities"
            );
        }
    }

    #[test]
    fn sourced_identity_and_nullable_link_keys_and_states_are_mandatory() {
        for path in [
            "/definitions/commodities/rows/0/guid",
            "/projections/commodityPresentation/rows/0/commodityGuid",
            "/projections/commodityPresentation/rows/3/subtypeGuid",
        ] {
            for invalid in [
                Value::Null,
                json!(id(1)),
                json!({"value":null}),
                json!({"state":"present"}),
                json!({"state":"absent","value":null}),
                json!({"state":"null","value":null}),
                json!({"state":"unreadable","value":null}),
            ] {
                let mut catalog = catalog();
                *catalog.pointer_mut(path).expect("synthetic field") = invalid;
                assert!(
                    check(&catalog).is_err(),
                    "counting requires a present sourced carrier and an explicit payload"
                );
            }
        }
        for (row, field) in [
            ("/definitions/commodities/rows/0", "guid"),
            ("/projections/commodityPresentation/rows/0", "commodityGuid"),
            ("/projections/commodityPresentation/rows/3", "subtypeGuid"),
        ] {
            let mut missing = catalog();
            assert!(
                missing
                    .pointer_mut(row)
                    .and_then(Value::as_object_mut)
                    .expect("synthetic row")
                    .remove(field)
                    .is_some(),
                "synthetic key must exist"
            );
            assert!(
                check(&missing).is_err(),
                "required identity and nullable keys must be authored explicitly"
            );
        }
    }

    #[test]
    fn source_carrier_origin_keys_are_required_without_changing_count_policy() {
        for field in [
            "sourceUsesRefs",
            "containsEncryptedSource",
            "sourceSetComplete",
        ] {
            let mut catalog = catalog();
            assert!(
                catalog["projections"]["commodityPresentation"]["rows"][3]["subtypeGuid"]
                    .as_object_mut()
                    .expect("synthetic sourced carrier")
                    .remove(field)
                    .is_some(),
                "synthetic origin key must exist"
            );
            assert!(
                check(&catalog).is_err(),
                "the canonical sourced wrapper must remain intact"
            );
        }
    }

    #[test]
    fn malformed_private_guid_errors_keep_the_existing_sanitized_boundary() {
        let mut catalog = catalog();
        catalog["definitions"]["commodities"]["rows"][0]["guid"]["value"] =
            json!("private-record-identity");
        let error = check(&catalog).expect_err("invalid synthetic GUID");
        assert!(!error.to_string().contains("private-record-identity"));
        assert!(!format!("{error:?}").contains("private-record-identity"));
    }
    #[test]
    fn both_canonical_datasets_must_be_available_bounded_nonempty_arrays() {
        for path in [
            "/definitions/commodities",
            "/projections/commodityPresentation",
        ] {
            for availability in [
                Value::Null,
                json!("absent"),
                json!("unavailable"),
                json!("future"),
            ] {
                let mut catalog = catalog();
                catalog.pointer_mut(path).expect("synthetic dataset")["availability"] =
                    availability;
                assert!(
                    check(&catalog).is_err(),
                    "unavailable data is not zero cardinality"
                );
            }
            for rows in [
                Value::Null,
                json!({}),
                json!([]),
                Value::Array(vec![Value::Null; MAX_ROWS + 1]),
            ] {
                let mut catalog = catalog();
                catalog.pointer_mut(path).expect("synthetic dataset")["rows"] = rows;
                assert!(
                    check(&catalog).is_err(),
                    "both row inventories obey the existing budget"
                );
            }
            let mut missing = catalog();
            *missing.pointer_mut(path).expect("synthetic dataset") = Value::Null;
            assert!(
                check(&missing).is_err(),
                "a root legacy array cannot supply a canonical dataset"
            );
        }
    }
}
