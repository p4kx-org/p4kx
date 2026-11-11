// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "real_archive_text_forms_mutations.rs"]
mod forms_mutations;
#[path = "real_archive_mission_locale_tests.rs"]
mod mission_locales;

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_export::Kind;
use p4kx_extraction::{Language, LocalizationSet};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use super::{Coverage, Oracle, check_coverage};
use crate::catalog_v3_consumer;
use crate::commodity_v3_fixture::{self as source, DcbOrigin};
use crate::real_archive_canonical_texts;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Fixture {
    catalog: Value,
    english: Oracle,
    german: Oracle,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let database = source::with_keys("@shared", "@shared", "@shared");
        Self::from_database(&database)
    }

    fn from_database(database: &DataForgeBuilder) -> TestResult<Self> {
        let (_folder, archive) = source::open(database, DcbOrigin::Plain)?;
        let built = p4kx_export::build_catalog(&archive, &source::options())?;
        let catalog = serde_json::from_slice(built.json())?;
        let locales = LocalizationSet::read(&archive, &[Language::English, Language::German])?;
        Ok(Self {
            catalog,
            english: Oracle::of(&locales, Language::English)?,
            german: Oracle::of(&locales, Language::German)?,
        })
    }

    fn coverage(&self, catalog: &Value, kind: Kind) -> TestResult<(Coverage, Coverage)> {
        real_archive_canonical_texts::kind_coverage(catalog, kind, &[], &self.english, &self.german)
    }
}

fn name_path(kind: Kind) -> TestResult<&'static str> {
    match kind {
        Kind::ResourceTypes => Ok("/definitions/resourceTypes/rows/0/name"),
        Kind::Commodities => Ok("/projections/commodityPresentation/rows/0/name"),
        _ => Err("test requires a canonical resource or Commodity".into()),
    }
}

fn reference(catalog: &Value, kind: Kind) -> TestResult<&str> {
    source::at(catalog, name_path(kind)?)?
        .get("value")
        .and_then(Value::as_str)
        .ok_or_else(|| "actual producer name reference".into())
}

fn name_value_mut(catalog: &mut Value, kind: Kind) -> TestResult<&mut Value> {
    catalog
        .pointer_mut(name_path(kind)?)
        .and_then(|carrier| carrier.get_mut("value"))
        .ok_or_else(|| "actual producer name payload".into())
}

fn names_mut(catalog: &mut Value) -> TestResult<&mut Vec<Value>> {
    catalog
        .pointer_mut("/texts/publicNames")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "actual public-name rows".into())
}

fn replace_value(value: &mut Value, pointer: &str, replacement: Value) -> TestResult {
    let target = value
        .pointer_mut(pointer)
        .ok_or_else(|| format!("required mutation target {pointer}"))?;
    *target = replacement;
    Ok(())
}

fn target<'catalog>(catalog: &'catalog mut Value, id: &str) -> TestResult<&'catalog mut Value> {
    names_mut(catalog)?
        .iter_mut()
        .find(|row| row.pointer("/id/value").and_then(Value::as_str) == Some(id))
        .ok_or_else(|| "actual producer target".into())
}

fn references(catalog: &Value, kind: Kind) -> TestResult<(usize, usize)> {
    let rows = if kind == Kind::Commodities {
        catalog_v3_consumer::presentations(catalog)?
    } else {
        catalog_v3_consumer::rows(catalog, kind)?
    };
    let fields: &[&str] = if kind == Kind::Commodities {
        &["name", "description", "displayType"]
    } else {
        &["name", "description"]
    };
    let mut occurrences = 0;
    let mut unique = BTreeSet::new();
    for row in rows {
        for &field in fields {
            if let Some(id) = row
                .get(field)
                .and_then(|carrier| carrier.get("value"))
                .and_then(Value::as_str)
            {
                occurrences += 1;
                unique.insert(id);
            }
        }
    }
    Ok((occurrences, unique.len()))
}

#[test]
fn real_acceptance_counts_canonical_resource_and_commodity_text_occurrences() -> TestResult {
    let fixture = Fixture::new()?;
    assert!(
        fixture.catalog.get("commodities").is_none(),
        "only canonical Commodity rows are emitted"
    );
    assert!(
        fixture.catalog.get("resourceTypes").is_none(),
        "only canonical ResourceType rows are emitted"
    );
    for kind in [Kind::ResourceTypes, Kind::Commodities] {
        let (english, german) = fixture.coverage(&fixture.catalog, kind)?;
        let (occurrences, unique) = references(&fixture.catalog, kind)?;
        assert!(
            occurrences > unique,
            "the source fixture repeats actual references"
        );
        assert_eq!(
            english.total(),
            occurrences,
            "coverage counts owners, not registry rows"
        );
        assert_eq!(
            german.total(),
            occurrences,
            "German coverage counts the same actual owner occurrences"
        );
        assert!(
            english.resolved > 0 && german.resolved > 0,
            "the actual source texts are compared"
        );
        assert_eq!(
            (english.contradictions, german.contradictions),
            (0, 0),
            "canonical translations agree with both independent source oracles"
        );
    }
    assert_eq!(
        fixture
            .coverage(&fixture.catalog, Kind::ResourceTypes)?
            .0
            .total(),
        6,
        "all three actual ResourceType rows contribute both text owners"
    );
    Ok(())
}

#[test]
fn present_null_is_a_nullable_commodity_choice_without_an_invented_lookup() -> TestResult {
    let fixture = Fixture::new()?;
    let mut nullable = fixture.catalog.clone();
    let before = fixture.coverage(&nullable, Kind::Commodities)?.0.total();
    *name_value_mut(&mut nullable, Kind::Commodities)? = Value::Null;
    assert_eq!(
        fixture.coverage(&nullable, Kind::Commodities)?.0.total(),
        before - 1,
        "a present-null choice contributes no invented text lookup"
    );
    let mut invalid = fixture.catalog.clone();
    *name_value_mut(&mut invalid, Kind::ResourceTypes)? = Value::Null;
    assert!(
        fixture.coverage(&invalid, Kind::ResourceTypes).is_err(),
        "non-nullable references require a string"
    );
    Ok(())
}

#[test]
fn canonical_domains_missing_targets_and_duplicates_are_not_coverage_successes() -> TestResult {
    let fixture = Fixture::new()?;
    for kind in [Kind::ResourceTypes, Kind::Commodities] {
        let id = reference(&fixture.catalog, kind)?;
        let mut missing = fixture.catalog.clone();
        names_mut(&mut missing)?
            .retain(|row| row.pointer("/id/value").and_then(Value::as_str) != Some(id));
        assert!(
            fixture.coverage(&missing, kind).is_err(),
            "missing canonical targets cannot become zero-key success"
        );
        let mut cross_domain = fixture.catalog.clone();
        let protected = source::at(&cross_domain, "/texts/gameTexts/0/id/value")?.clone();
        assert!(protected.is_string(), "a real protected target exists");
        *name_value_mut(&mut cross_domain, kind)? = protected;
        assert!(
            fixture.coverage(&cross_domain, kind).is_err(),
            "public owner references cannot use protected targets"
        );
        let mut duplicate = fixture.catalog.clone();
        let repeated = target(&mut duplicate, id)?.clone();
        names_mut(&mut duplicate)?.push(repeated);
        assert!(
            fixture.coverage(&duplicate, kind).is_err(),
            "duplicate canonical identities are ambiguous"
        );
    }
    Ok(())
}

#[test]
fn missing_canonical_language_is_rejected() -> TestResult {
    let fixture = Fixture::new()?;
    for kind in [Kind::ResourceTypes, Kind::Commodities] {
        let id = reference(&fixture.catalog, kind)?;
        let mut missing = fixture.catalog.clone();
        target(&mut missing, id)?
            .pointer_mut("/translations/value")
            .and_then(Value::as_object_mut)
            .ok_or("language map")?
            .remove("de")
            .ok_or("actual German carrier")?;
        assert!(
            fixture.coverage(&missing, kind).is_err(),
            "a required language carrier cannot disappear"
        );
    }
    Ok(())
}

#[test]
fn canonical_owner_requires_state_and_origin_metadata() -> TestResult {
    let fixture = Fixture::new()?;
    for kind in [Kind::ResourceTypes, Kind::Commodities] {
        for field in ["state", "containsEncryptedSource"] {
            let mut missing = fixture.catalog.clone();
            missing
                .pointer_mut(name_path(kind)?)
                .and_then(Value::as_object_mut)
                .ok_or("name carrier")?
                .remove(field)
                .ok_or("required source metadata")?;
            assert!(
                fixture.coverage(&missing, kind).is_err(),
                "immediate source metadata is required"
            );
        }
    }
    forms_mutations::require_metadata(&fixture)?;
    Ok(())
}

#[test]
fn missing_canonical_dataset_is_not_zero_key_coverage() -> TestResult {
    let fixture = Fixture::new()?;
    for kind in [Kind::ResourceTypes, Kind::Commodities] {
        let mut missing = fixture.catalog.clone();
        let field = if kind == Kind::ResourceTypes {
            "resourceTypes"
        } else {
            "commodities"
        };
        missing
            .get_mut("definitions")
            .and_then(Value::as_object_mut)
            .ok_or("definitions")?
            .remove(field)
            .ok_or("actual canonical dataset")?;
        assert!(
            fixture.coverage(&missing, kind).is_err(),
            "a canonical source dataset is required"
        );
    }
    Ok(())
}

#[test]
fn wrong_or_falsely_missing_canonical_translations_are_contradictions() -> TestResult {
    forms_mutations::wrong_or_missing()
}

fn omit_references(catalog: &mut Value, kind: Kind) -> TestResult<usize> {
    let count = catalog_v3_consumer::rows(catalog, kind)?.len();
    let (pointer, fields, state): (&str, &[&str], &str) = if kind == Kind::Commodities {
        (
            "/projections/commodityPresentation/rows",
            &["name", "description", "displayType"],
            "present",
        )
    } else {
        (
            "/definitions/resourceTypes/rows",
            &["name", "description"],
            "absent",
        )
    };
    for row in catalog
        .pointer_mut(pointer)
        .and_then(Value::as_array_mut)
        .ok_or("actual rows")?
    {
        for &field in fields {
            let carrier = row.get_mut(field).ok_or("required canonical owner field")?;
            replace_value(carrier, "/state", json!(state))?;
            replace_value(carrier, "/value", Value::Null)?;
        }
    }
    Ok(count)
}

#[test]
fn positive_baseline_families_cannot_disappear_into_zero_key_coverage() -> TestResult {
    let fixture = Fixture::new()?;
    for kind in [Kind::ResourceTypes, Kind::Commodities] {
        let mut empty_refs = fixture.catalog.clone();
        let rows = omit_references(&mut empty_refs, kind)?;
        let (english, german) = fixture.coverage(&empty_refs, kind)?;
        assert_eq!(
            english.total(),
            0,
            "the mutation removes every source reference"
        );
        let mut failures = Vec::new();
        check_coverage(kind, &english, &german, true, rows, &mut failures);
        assert_eq!(
            failures.len(),
            1,
            "positive baseline rows require exercised text coverage"
        );
        failures.clear();
        check_coverage(kind, &english, &german, false, rows, &mut failures);
        assert!(
            failures.is_empty(),
            "unbaselined source absence invents no text floor"
        );
    }
    Ok(())
}
