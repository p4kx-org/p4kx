// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{
    Archive, DataForgeOrigin, InputScope, SourceInput, SourceLocation, SourceScope, SourceUseRole,
    SourceUses, SourceValue, Value as Raw,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::super::carriers::{Carriers, Uses};
use super::super::{Failure, Result};
use super::{Coverage, Walker};

struct Fixture {
    _folder: tempfile::TempDir,
    origin: DataForgeOrigin,
    sources: SourceUses,
    scope: SourceScope,
}

impl Fixture {
    fn new() -> Result<Self> {
        let folder =
            tempfile::tempdir().map_err(|error| Failure::source("synthetic-folder", error))?;
        let path = folder.path().join("source.p4k");
        ArchiveBuilder::new()
            .entry(TestEntry::stored("Data/Game2.dcb", b"synthetic-payload"))
            .write_to(&path)
            .map_err(|error| Failure::source("synthetic-archive", error))?;
        let archive = Archive::open(path)
            .map_err(|error| Failure::source("synthetic-archive-open", error))?;
        let entry = archive
            .entries()
            .first()
            .ok_or_else(|| Failure::check("synthetic-entry"))?;
        let origin = archive
            .entry_origin(entry)
            .ok_or_else(|| Failure::check("synthetic-origin"))?;
        let sources = SourceUses::new();
        let scope = sources
            .observe(SourceInput::Archive(&origin), InputScope::Payload)
            .map_err(|error| Failure::source("synthetic-scope", error))?;
        Ok(Self {
            _folder: folder,
            origin: DataForgeOrigin::Archive(origin),
            sources,
            scope,
        })
    }

    fn carrier(&self, path: &str, value: Value) -> Result<Value> {
        let used = self
            .scope
            .at(
                SourceLocation::DataForge {
                    record_index: 0,
                    field: path.to_owned(),
                },
                SourceUseRole::Payload,
            )
            .map_err(|error| Failure::source("synthetic-use", error))?;
        serde_json::to_value(
            self.sources
                .value(SourceValue::Present(value), &[used], true),
        )
        .map_err(|error| Failure::source("synthetic-carrier", error))
    }

    fn catalog(&self) -> Result<Value> {
        let inventory = self
            .sources
            .freeze()
            .map_err(|error| Failure::source("synthetic-inventory", error))?;
        Ok(json!({"provenance": inventory}))
    }

    fn check(&self, row: &Value, raw: &Raw<'_>, path: &str) -> Result<()> {
        let catalog = self.catalog()?;
        self.check_catalog(row, raw, path, &catalog)
    }

    fn check_catalog(&self, row: &Value, raw: &Raw<'_>, path: &str, catalog: &Value) -> Result<()> {
        let uses = Uses::new(catalog, &self.origin)?;
        let carriers = Carriers::new(row, &uses, 0)?;
        let mut coverage = Coverage::default();
        let mut fields = BTreeSet::new();
        Walker {
            carriers: &carriers,
            coverage: &mut coverage,
            fields: &mut fields,
        }
        .walk(Some(raw), path, 0)
    }
}

#[test]
fn a_correct_additive_carrier_cannot_mask_the_wrong_canonical_boolean() -> Result<()> {
    let fixture = Fixture::new()?;
    let good = fixture.carrier("validateDefaultCargoBox", json!(true))?;
    let mut row = json!({"validateDefaultCargoBox": good});
    fixture.check(&row, &Raw::Bool(true), "validateDefaultCargoBox")?;
    row["extra"] = row["validateDefaultCargoBox"].clone();
    row["validateDefaultCargoBox"]["value"] = json!(false);
    assert!(
        fixture
            .check(&row, &Raw::Bool(true), "validateDefaultCargoBox")
            .is_err(),
        "the canonical field cannot borrow a matching value from an additive sibling"
    );
    Ok(())
}

#[test]
fn swapped_nested_values_cannot_borrow_the_other_parent_carrier() -> Result<()> {
    let fixture = Fixture::new()?;
    let left = fixture.carrier("properties[0]", json!(true))?;
    let right = fixture.carrier("properties[1]", json!(false))?;
    let array = fixture.carrier("properties", json!([left, right]))?;
    let mut row = json!({"properties": array});
    let raw = Raw::Array(Box::new([Raw::Bool(true), Raw::Bool(false)]));
    fixture.check(&row, &raw, "properties")?;
    row["extra"] = row["properties"].clone();
    row["properties"]["value"][0]["value"] = json!(false);
    row["properties"]["value"][1]["value"] = json!(true);
    assert!(
        fixture.check(&row, &raw, "properties").is_err(),
        "the selected array elements cannot borrow matching carriers from another parent"
    );
    Ok(())
}

#[test]
fn attempted_observations_cannot_contribute_to_a_present_field() -> Result<()> {
    let fixture = Fixture::new()?;
    let row = json!({"validateDefaultCargoBox":
        fixture.carrier("validateDefaultCargoBox", json!(true))?});
    let mut catalog = fixture.catalog()?;
    fixture.check_catalog(&row, &Raw::Bool(true), "validateDefaultCargoBox", &catalog)?;
    let uses = catalog
        .pointer_mut("/provenance/uses")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| Failure::check("synthetic-uses"))?;
    for used in uses {
        used["role"] = json!("attempted");
    }
    assert!(
        fixture
            .check_catalog(&row, &Raw::Bool(true), "validateDefaultCargoBox", &catalog)
            .is_err(),
        "a failed or planned read cannot justify the emitted value"
    );
    Ok(())
}

#[test]
fn a_wrong_immediate_encryption_marker_is_rejected() -> Result<()> {
    let fixture = Fixture::new()?;
    let mut row = json!({"validateDefaultCargoBox":
        fixture.carrier("validateDefaultCargoBox", json!(true))?});
    fixture.check(&row, &Raw::Bool(true), "validateDefaultCargoBox")?;
    let actual = row["validateDefaultCargoBox"]["containsEncryptedSource"].as_bool();
    row["validateDefaultCargoBox"]["containsEncryptedSource"] = json!(actual != Some(true));
    assert!(
        fixture
            .check(&row, &Raw::Bool(true), "validateDefaultCargoBox")
            .is_err(),
        "the immediate index must match the actual source contribution"
    );
    Ok(())
}

#[test]
fn a_missing_immediate_encryption_marker_is_rejected() -> Result<()> {
    reject_missing_origin_key("containsEncryptedSource")
}

#[test]
fn a_missing_source_set_completeness_key_is_rejected() -> Result<()> {
    reject_missing_origin_key("sourceSetComplete")
}

fn reject_missing_origin_key(key: &str) -> Result<()> {
    let fixture = Fixture::new()?;
    let mut row = json!({"validateDefaultCargoBox":
        fixture.carrier("validateDefaultCargoBox", json!(true))?});
    fixture.check(&row, &Raw::Bool(true), "validateDefaultCargoBox")?;
    row["validateDefaultCargoBox"]
        .as_object_mut()
        .ok_or_else(|| Failure::check("synthetic-carrier-object"))?
        .remove(key);
    assert!(
        fixture
            .check(&row, &Raw::Bool(true), "validateDefaultCargoBox")
            .is_err(),
        "an absent origin key must not masquerade as an explicit unknown"
    );
    Ok(())
}

#[test]
fn a_false_source_set_completeness_claim_is_rejected() -> Result<()> {
    let fixture = Fixture::new()?;
    let mut row = json!({"validateDefaultCargoBox":
        fixture.carrier("validateDefaultCargoBox", json!(true))?});
    fixture.check(&row, &Raw::Bool(true), "validateDefaultCargoBox")?;
    row["validateDefaultCargoBox"]["sourceSetComplete"] = json!(false);
    assert!(
        fixture
            .check(&row, &Raw::Bool(true), "validateDefaultCargoBox")
            .is_err(),
        "the successful complete source read must retain its complete contributor inventory"
    );
    Ok(())
}
