// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::cell::Cell;

use p4kx_export::catalog_queries::equipment::{ItemSnapshot, with_items};
use p4kx_export::catalog_queries::{CatalogRecordSelection, DatasetAvailability};
use p4kx_export::{CatalogError, CatalogOptions, build_catalog};
use p4kx_extraction::{
    Archive, CancelToken, FieldOrigin, Filter, InputScope, Language, PhysicalSourceKind,
    SourceEncryptionState, SourceInventory, SourceLocation,
};
use serde_json::{Value, json};

use crate::catalog_variants::decision_reader::variant_cell;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::variants_fixture::item_rank as fixture;

#[test]
fn filtered_mission_item_keeps_the_player_variant_base_outside_output() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let full = normal(&archive, None)?;
    let expected_base = json!(common::guid(fixture::BASE_GUID));
    check_full_group(&full, &expected_base)?;

    let filter = Filter::new(fixture::SELECTED_FILTER);
    let filtered = normal(&archive, Some(&filter))?;
    let [selected] = proof::rows(&filtered, "/items")? else {
        return Err("the Normal filter must emit one original Item".into());
    };
    assert_eq!(
        proof::at(selected, "/recordIndex/value")?,
        &json!(fixture::SELECTED_RECORD_INDEX),
        "the filtered row is the same physical mission Item"
    );
    assert_eq!(
        proof::at(selected, "/playability/value")?,
        &json!("mission"),
        "the selected row retains its original non-player ranking"
    );
    assert_player_base(selected, &expected_base)?;
    proof::closure(&filtered)?;
    check_sdk(&archive, &full, &filtered, selected, &expected_base)
}

fn check_sdk(
    archive: &Archive,
    full: &Value,
    filtered: &Value,
    filtered_row: &Value,
    expected_base: &Value,
) -> TestResult {
    let full_row = at_record(proof::rows(full, "/items")?, fixture::SELECTED_RECORD_INDEX)?;
    full_proof::open(archive, full_proof::NO_LANGUAGES, None)?.with_items(
        CatalogRecordSelection::RecordIndex(fixture::SELECTED_RECORD_INDEX),
        None,
        |snapshot| {
            check_sdk_scope(&snapshot)?;
            let [row] = snapshot.rows() else {
                return Err("one selected physical Item SDK row".into());
            };
            let decision = row.variant_of();
            let target = decision
                .value()
                .ok_or("the original winning physical Item")?;
            let guid = target.guid();
            check_origin(snapshot.source_inventory(), decision.origin())?;
            check_origin(
                snapshot.source_inventory(),
                target.target_record_index().origin(),
            )?;
            check_origin(snapshot.source_inventory(), guid.origin())?;
            check_target_coordinate(
                snapshot.source_inventory(),
                target.target_record_index().origin(),
            );

            let actual = serde_json::to_value(row)?;
            assert_player_base(&actual, expected_base)?;
            let sdk_set = verify_sdk_set(&snapshot, &actual)?;
            let sdk = json!({ "provenance": snapshot.provenance() });
            assert_eq!(
                sdk.pointer("/provenance/variantSourceUseSets/0"),
                Some(&sdk_set),
                "the query serializes its shared decision set in the canonical provenance"
            );
            let observed = proof::with_origins(actual, &sdk)?;
            for (label, document, expected) in [
                ("full", full, full_row),
                ("filtered", filtered, filtered_row),
            ] {
                assert_eq!(
                    observed,
                    proof::with_origins(expected.clone(), document)?,
                    "the complete selected SDK Item and physical origins match {label} Normal"
                );
                let expected_set = catalog_set(document, fixture::OTHER_PLAYER_RECORD_INDEX)?;
                assert_eq!(
                    proof::with_origins(sdk_set.clone(), &sdk)?,
                    proof::with_origins(expected_set.clone(), document)?,
                    "the pooled physical proof matches {label} Normal"
                );
            }
            Ok(())
        },
    )?
}

fn verify_sdk_set(snapshot: &ItemSnapshot<'_>, actual: &Value) -> TestResult<Value> {
    let set_id = fixture::OTHER_PLAYER_RECORD_INDEX;
    let [row] = snapshot.rows() else {
        return Err("one selected physical Item".into());
    };
    assert_eq!(
        row.variant_source_use_set_ref(),
        Some(set_id),
        "the minimum comparable Record outside the selection anchors the set"
    );
    let sets = snapshot.variant_source_use_sets();
    assert_eq!(
        sets.iter_sets().len(),
        1,
        "only the emitted cohort is retained"
    );
    let set = sets.get(set_id).ok_or("the referenced cohort")?;
    let local = proof::at(actual, "/variantOf/sourceUsesRefs")?
        .as_array()
        .ok_or("local variant use references")?;
    let expanded = proof::at(&serde_json::to_value(row.variant_of())?, "/sourceUsesRefs")?.clone();
    let mut union: Vec<_> = serde_json::to_value(set.source_uses_refs())?
        .as_array()
        .ok_or("actual pooled refs")?
        .clone();
    union.extend(local.iter().cloned());
    union.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
    assert_eq!(
        json!(union),
        expanded,
        "the disjoint compact and local refs are exactly the typed full origin"
    );
    assert!(
        local.len() < set.source_uses_refs().len(),
        "the selected row omits the shared cohort references"
    );
    Ok(serde_json::to_value(set)?)
}

fn catalog_set(catalog: &Value, id: u32) -> TestResult<&Value> {
    proof::rows(catalog, "/provenance/variantSourceUseSets")?
        .iter()
        .find(|set| set.get("id").and_then(Value::as_u64) == Some(u64::from(id)))
        .ok_or_else(|| "exact shared group proof missing in Catalog".into())
}

fn check_sdk_scope(snapshot: &ItemSnapshot<'_>) -> TestResult {
    assert!(
        matches!(snapshot.availability(), DatasetAvailability::Available),
        "the Item family contains the selected physical source"
    );
    assert!(
        snapshot.coverage().unreadable_headers.is_empty()
            && snapshot.coverage().unreadable_bodies.is_empty()
            && snapshot.coverage().incomplete_fields.is_empty(),
        "the selected owner and its variant decision are fully observed"
    );
    assert!(
        snapshot.requested_languages().is_empty()
            && snapshot.available_languages().is_empty()
            && snapshot.public_names().is_empty()
            && snapshot.game_texts().is_empty(),
        "the Item rank query opens no presentation language or text dataset"
    );
    for source in snapshot.source_inventory().iter_sources() {
        if let PhysicalSourceKind::ArchiveEntry(entry) = source.kind() {
            let path = entry
                .path()
                .value()
                .ok_or("actual SDK Archive Entry path")?;
            assert!(
                path.eq_ignore_ascii_case("Data\\Game2.dcb")
                    || Language::from_global_ini_path(path) == Some(Language::English),
                "only the original DataForge and semantic English entries are read"
            );
        }
    }
    Ok(())
}

fn check_origin(inventory: &SourceInventory, origin: &FieldOrigin) -> TestResult {
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "the actual captured DCB inputs are encrypted"
    );
    assert!(
        origin.source_set_complete(),
        "the variant Datum is fully sourced"
    );
    assert!(
        !origin.source_uses_refs().is_empty(),
        "the decision retains its actual source uses"
    );
    for reference in origin.source_uses_refs() {
        let used = inventory
            .iter_uses()
            .find(|used| used.id() == reference)
            .ok_or("original Item SDK source-use reference")?;
        assert_eq!(
            used.scope(),
            InputScope::Payload,
            "the evidenced DCB byte domain is payload"
        );
        assert_eq!(
            used.effective_encryption(),
            SourceEncryptionState::Encrypted,
            "the use has actual stored-source lineage"
        );
        let source = inventory
            .iter_sources()
            .find(|source| source.id() == used.source())
            .ok_or("original physical SDK source")?;
        assert!(
            matches!(source.kind(), PhysicalSourceKind::ArchiveEntry(entry)
                if entry.path().value().is_some_and(|path| path.eq_ignore_ascii_case("Data\\Game2.dcb"))),
            "the operand closes to the original Archive DCB Entry"
        );
    }
    Ok(())
}

fn check_target_coordinate(inventory: &SourceInventory, origin: &FieldOrigin) {
    let index = u64::from(fixture::BASE_RECORD_INDEX);
    assert!(
        inventory.iter_uses().any(|used| {
            origin.source_uses_refs().contains(used.id())
                && matches!(used.location(), SourceLocation::Subresource { namespace, indices, field }
                    if namespace == "dataforgeRecordHeader"
                        && indices.as_slice() == [index]
                        && field == "recordIndex")
        }),
        "the typed target RecordIndex cites the chosen alias's exact physical header"
    );
}

#[test]
fn cancelled_item_root_query_never_delivers_a_row() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Encrypted)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let called = Cell::new(false);
    let result = with_items(
        &archive,
        CatalogRecordSelection::RecordIndex(fixture::SELECTED_RECORD_INDEX),
        Some(&cancel),
        |_| called.set(true),
    );
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "pre-cancel is reported"
    );
    assert!(
        !called.get(),
        "a cancelled query does not enter the consumer callback"
    );
    Ok(())
}

fn normal(archive: &Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let options = CatalogOptions {
        filter,
        ..common::options()
    };
    let built = build_catalog(archive, options.with_full_provenance())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn check_full_group(catalog: &Value, expected_base: &Value) -> TestResult {
    let rows = proof::rows(catalog, "/items")?;
    assert_eq!(
        rows.len(),
        5,
        "the fixture retains every physical candidate"
    );
    let selected = at_record(rows, fixture::SELECTED_RECORD_INDEX)?;
    let player = at_record(rows, fixture::BASE_RECORD_INDEX)?;
    let tied_alias = at_record(rows, fixture::EQUAL_RANK_RECORD_INDEX)?;
    let other_player = at_record(rows, fixture::OTHER_PLAYER_RECORD_INDEX)?;
    let wrong_key_alias = at_record(rows, fixture::WRONG_KEY_RECORD_INDEX)?;
    check_full_set(
        catalog,
        [player, tied_alias, other_player, selected],
        wrong_key_alias,
    )?;
    for alias in [player, tied_alias, wrong_key_alias] {
        assert_eq!(
            proof::at(alias, "/guid/value")?,
            expected_base,
            "the physical alias has the nominated common GUID"
        );
        let absent = variant_cell(alias, "absent")?;
        assert_eq!(
            absent.get("sourceSetComplete").and_then(Value::as_bool),
            Some(true),
            "the known physical aliases do not link to their own GUID"
        );
    }
    assert_eq!(
        proof::at(player, "/className/value")?,
        proof::at(tied_alias, "/className/value")?,
        "the two group-member aliases have the same lexical rank"
    );
    assert_eq!(
        proof::at(player, "/playability/value")?,
        &json!("player"),
        "the authored purchase key establishes player rank"
    );
    assert_player_base(other_player, expected_base)?;
    assert_player_base(selected, expected_base)?;
    proof::closure(catalog)
}

fn check_full_set(catalog: &Value, matching: [&Value; 4], distinct: &Value) -> TestResult {
    let [set] = proof::rows(catalog, "/provenance/variantSourceUseSets")? else {
        return Err("one shared set for the four physical cohort members".into());
    };
    let anchor = json!(fixture::OTHER_PLAYER_RECORD_INDEX);
    assert_eq!(
        proof::at(set, "/id")?,
        &anchor,
        "the smallest comparable physical Record anchors the set"
    );
    assert_eq!(
        proof::at(distinct, "/variantOf/sourceUseSetRef")?,
        &Value::Null,
        "the physical alias with a distinct key uses its own complete proof"
    );
    let shared = proof::rows(set, "/sourceUsesRefs")?;
    for row in matching {
        assert_eq!(
            proof::at(row, "/variantOf/sourceUseSetRef")?,
            &anchor,
            "each emitted matching row references the same physical set"
        );
        let local = proof::rows(row, "/variantOf/sourceUsesRefs")?;
        assert!(
            local.len() < shared.len() && local.iter().all(|reference| !shared.contains(reference)),
            "the row retains only local references outside the pooled set"
        );
    }
    Ok(())
}

fn assert_player_base(row: &Value, expected_guid: &Value) -> TestResult {
    let decision = variant_cell(row, "present")?;
    assert_eq!(
        decision.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "the winner is proven against the unfiltered Item candidates"
    );
    assert_eq!(
        decision
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "the selected variant decision retains its encrypted DataForge contributor"
    );
    let target = proof::at(decision, "/value")?;
    proof::present(target, "guid", expected_guid, Some(true))?;
    proof::present(
        target,
        "targetRecordIndex",
        &json!(fixture::BASE_RECORD_INDEX),
        Some(true),
    )?;
    Ok(())
}

fn at_record(rows: &[Value], physical_index: u32) -> TestResult<&Value> {
    rows.iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64)
                == Some(u64::from(physical_index))
        })
        .ok_or_else(|| format!("missing fixture Record {physical_index}").into())
}
