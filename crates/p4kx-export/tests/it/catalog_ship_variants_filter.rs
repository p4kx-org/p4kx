// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{ShipSnapshot, with_ships};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, Filter, SourceValueState};
use serde_json::{Value, json};

use super::{EXPORTED_AT, UNIX_EPOCH};
use crate::commodity_v3_fixture::TestResult;
use crate::health_origin_assertions as proof;
use crate::variants_fixture::{self, read_rows};

const SELECTED: &str = "ACAS_Alpha_Edition";
const BASE: &str = "ACAS_Alpha";

#[test]
fn filtered_ship_and_sdk_keep_the_same_original_base_and_sources() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        variants_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let full = normal(&archive, None)?;
    let intended = read_rows::entry(&full, "ships", SELECTED)?;
    let physical = proof::at(intended, "/recordIndex/value")?
        .as_u64()
        .ok_or("the original selected Record index")?;
    let base = read_rows::entry(&full, "ships", BASE)?;
    let base_index = proof::at(base, "/recordIndex/value")?
        .as_u64()
        .ok_or("the original base Record index")?;
    let filter = Filter::new("acas_alpha_edition");
    let filtered = normal(&archive, Some(&filter))?;
    let [selected] = proof::rows(&filtered, "/ships")? else {
        return Err("the path filter should emit only the selected ship".into());
    };
    let full_proof = canonical_variant(&full, intended)?;
    let filtered_proof = canonical_variant(&filtered, selected)?;
    assert_eq!(
        filtered_proof, full_proof,
        "a path filter preserves the complete physical decision and its source receipts"
    );
    with_ships(
        &archive,
        CatalogRecordSelection::RecordIndex(u32::try_from(physical)?),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| check_sdk(&snapshot, &full_proof, base_index),
    )?
}

fn normal(archive: &Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let built = build_catalog(
        archive,
        &CatalogOptions {
            filter,
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
        },
    )?;
    Ok(serde_json::from_slice(built.json())?)
}

fn canonical_variant(catalog: &Value, row: &Value) -> TestResult<Value> {
    let mut decision = proof::at(row, "/variantOf")?.clone();
    let object = decision
        .as_object_mut()
        .ok_or("the original decision envelope")?;
    let set = object
        .remove("sourceUseSetRef")
        .and_then(|value| value.as_u64())
        .ok_or("the original pooled physical comparison")?;
    object.remove("failedSourceUseRefs");
    let expected = proof::rows(catalog, "/provenance/variantSourceUseSets")?
        .iter()
        .find(|row| row.get("id").and_then(Value::as_u64) == Some(set))
        .ok_or("the selected decision's pooled evidence")?;
    let refs = object
        .get_mut("sourceUsesRefs")
        .and_then(Value::as_array_mut)
        .ok_or("the local original refs")?;
    refs.extend(
        proof::at(expected, "/sourceUsesRefs")?
            .as_array()
            .ok_or("the pooled original refs")?
            .iter()
            .cloned(),
    );
    refs.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
    proof::with_origins(decision, catalog)
}

fn check_sdk(snapshot: &ShipSnapshot<'_>, expected: &Value, base_index: u64) -> TestResult {
    let [ship] = snapshot.rows() else {
        return Err("one selected physical Ship SDK row".into());
    };
    let decision = ship
        .variant_of()
        .ok_or("the canonical Ship route compares variants")?;
    assert_eq!(
        decision.state(),
        SourceValueState::Present,
        "the SDK locates the true unfiltered winner"
    );
    let target = decision.value().ok_or("the physical base source")?;
    assert_eq!(
        target.target_record_index().value().copied().map(u64::from),
        Some(base_index),
        "the winner is the physical Record outside the requested output"
    );
    assert!(
        ship.variant_failed_source_uses().is_empty(),
        "complete winning input scope contains no failed accesses"
    );
    let provenance = json!({ "provenance": snapshot.provenance() });
    let actual = serde_json::to_value(decision)?;
    assert_eq!(
        proof::with_origins(actual, &provenance)?,
        *expected,
        "the SDK returns the same resolved variant and original physical contributors"
    );
    Ok(())
}
