// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::equipment::{ShipSnapshot, ShipSource, with_ships};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{
    CatalogError, CatalogOptions, CatalogPublicationMode, ExportDiagnosticCode, Kind, build_catalog,
};
use p4kx_extraction::{Archive, Filter, SourceLocation, SourceUseRole, SourceValueState};
use serde_json::{Value, json};

use super::physical_fixture::{
    self, PATCH_BASE_CLASS, PATCH_OWNER_CLASS, PATCH_PEERS, PATCH_VARIANT_CLASS,
};
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
    full_proof::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?.with_ships(
        CatalogRecordSelection::RecordIndex(u32::try_from(physical)?),
        None,
        |snapshot| check_sdk(&snapshot, &full_proof, base_index),
    )?
}

#[test]
fn proved_missing_explicit_patch_excludes_only_its_original_ship() -> TestResult {
    let (_folder, archive) = physical_fixture::open_explicit_patch(None)?;
    let filter = Filter::new(PATCH_PEERS);
    let built = build_catalog(
        &archive,
        CatalogOptions {
            filter: Some(&filter),
            ..CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
        }
        .with_full_provenance(),
    )?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        proof::rows(&catalog, "/ships")?.len(),
        2,
        "only healthy peers are emitted"
    );
    let base = read_rows::entry(&catalog, "ships", PATCH_BASE_CLASS)?;
    let base_index = proof::at(base, "/recordIndex/value")?
        .as_u64()
        .ok_or("the physical base Record index")?;
    let variant = read_rows::entry(&catalog, "ships", PATCH_VARIANT_CLASS)?;
    let expected = canonical_variant(&catalog, variant)?;

    full_proof::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?.with_ships(
        CatalogRecordSelection::All,
        None,
        |snapshot| {
            let omitted = sdk_ship(&snapshot, PATCH_OWNER_CLASS)?;
            let decision = omitted.variant_of().ok_or("the excluded Ship decision")?;
            assert_eq!(
                decision.state(),
                SourceValueState::Absent,
                "the exact authored patch is absent"
            );
            assert!(
                decision.origin().source_set_complete(),
                "the exclusion has closed source evidence"
            );
            assert!(
                omitted.variant_failed_source_uses().is_empty(),
                "verified absence is not a failed payload read"
            );
            let used = snapshot
                .sources()
                .source_inventory()
                .select_uses(decision.origin().source_uses_refs())?;
            assert!(
                used.iter_uses()
                    .any(|source| source.role() == SourceUseRole::Selector
                        && matches!(
                            source.location(),
                            SourceLocation::ArchiveDirectoryPaths { .. }
                        )),
                "the original directory provides the negative lookup evidence"
            );
            let variant = sdk_ship(&snapshot, PATCH_VARIANT_CLASS)?;
            check_sdk_row(&snapshot, variant, &expected, base_index)
        },
    )?
}

#[test]
fn malformed_explicit_patch_keeps_the_original_cohort_unreadable() -> TestResult {
    let (_folder, archive) = physical_fixture::open_explicit_patch(Some(b"CryXmlB\0not really"))?;
    let filter = Filter::new(PATCH_PEERS);
    let Err(CatalogError::IncompleteExport(refusal)) = build_catalog(
        &archive,
        CatalogOptions {
            filter: Some(&filter),
            ..CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
        }
        .with_full_provenance(),
    ) else {
        return Err(
            "Normal publication must refuse an unresolved same-definition candidate".into(),
        );
    };
    assert!(
        refusal
            .health()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.kind == Some(Kind::Ships)
                && diagnostic.code == ExportDiagnosticCode::DerivedProjectionFailure
                && diagnostic
                    .examples()
                    .iter()
                    .any(|example| example.field_text().value() == Some("variantOf"))),
        "the healthy output scope refuses the unresolved variant decision"
    );

    with_ships(
        &archive,
        CatalogRecordSelection::All,
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            let selected = sdk_ship(&snapshot, PATCH_OWNER_CLASS)?;
            let decision = selected.variant_of().ok_or("the malformed Ship decision")?;
            assert_eq!(
                decision.state(),
                SourceValueState::Unreadable,
                "an attempted malformed patch cannot prove absence"
            );
            assert!(
                !decision.origin().source_set_complete(),
                "the failed payload leaves the sources incomplete"
            );
            assert!(
                !selected.variant_failed_source_uses().is_empty(),
                "the failed patch remains separately attributable"
            );
            Ok(())
        },
    )?
}

fn sdk_ship<'snapshot, 'source>(
    snapshot: &'snapshot ShipSnapshot<'source>,
    class: &str,
) -> TestResult<&'snapshot ShipSource<'source>> {
    snapshot
        .rows()
        .iter()
        .find(|ship| ship.source().class_name().value().copied() == Some(class))
        .ok_or_else(|| format!("missing physical Ship {class}").into())
}

fn normal(archive: &Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let built = build_catalog(
        archive,
        CatalogOptions {
            filter,
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
        }
        .with_full_provenance(),
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
    check_sdk_row(snapshot, ship, expected, base_index)
}

fn check_sdk_row(
    snapshot: &ShipSnapshot<'_>,
    ship: &ShipSource<'_>,
    expected: &Value,
    base_index: u64,
) -> TestResult {
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
