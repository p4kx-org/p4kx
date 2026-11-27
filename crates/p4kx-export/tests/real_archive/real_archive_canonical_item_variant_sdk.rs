// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::slice;

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::{Item, ItemSnapshot, with_items_from};
use p4kx_extraction::{
    Archive, Checksum, DataForge, DataForgeEntry, FieldOrigin, Guid, SourceInventory,
    SourceLocation, SourceUseRole, SourceValueState,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::equipment_input::{dataforge, with_archive};
use super::variant_parity;
use super::{complete, literal_filter};
use crate::real_archive_catalog_table_parity::{Origins, TestResult, wire_row};
use crate::real_archive_verification::print_line;

const TABLE: &str = "/items";

struct Selection {
    owner: u32,
    owner_path: String,
    base: u32,
    base_path: String,
}

impl Selection {
    fn configured() -> TestResult<Self> {
        let owner = original_index("P4KX_ITEM_VARIANT_RECORD")?;
        let owner_path = original_path("P4KX_ITEM_VARIANT_RECORD_PATH")?;
        let base = original_index("P4KX_ITEM_VARIANT_BASE_RECORD")?;
        let base_path = original_path("P4KX_ITEM_VARIANT_BASE_RECORD_PATH")?;
        if owner == base || owner_path == base_path {
            return Err(
                "the selected Item and its external base must be physically distinct".into(),
            );
        }
        Ok(Self {
            owner,
            owner_path,
            base,
            base_path,
        })
    }

    fn authenticate(&self, input: &DataForgeEntry) -> TestResult<OriginalGuids> {
        let forge = input.open().map_err(|_| "parse the bound Item DataForge")?;
        let owner = bind(&forge, self.owner, &self.owner_path)?;
        let base = bind(&forge, self.base, &self.base_path)?;
        if owner == base {
            return Err("an Item and its external base must have distinct GUIDs".into());
        }
        Ok(OriginalGuids { owner, base })
    }
}

fn original_index(variable: &str) -> TestResult<u32> {
    env::var(variable)
        .ok()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| "configure the private physical Item and base indices".into())
}

fn original_path(variable: &str) -> TestResult<String> {
    let path = env::var(variable).map_err(|_| "configure the two original Item paths")?;
    if path.is_empty() || path.contains(['*', '?']) {
        return Err("the nominated original Item paths must be literal".into());
    }
    Ok(path)
}

fn bind(forge: &DataForge<'_>, index: u32, path: &str) -> TestResult<Guid> {
    let record = forge
        .record(index)
        .ok_or("a nominated physical Item Record is absent")?;
    let found = record
        .record_path()
        .map_err(|_| "a nominated physical Item path is unreadable")?;
    if found != path
        || record.type_name().ok() != Some(p4kx_assembly::ENTITY_TYPE)
        || !forge
            .records_at_path(path)
            .map(|original| original.index())
            .eq([index])
    {
        return Err("a nominated Item path does not bind to one exact physical Entity".into());
    }
    Ok(record.guid())
}

struct OriginalGuids {
    owner: Guid,
    base: Guid,
}

struct SelectedItem {
    row: Value,
    set: Value,
    origins: Origins,
}

#[test]
#[ignore = "real Archive: run one physical Item variant cold in its own measured process"]
fn installed_canonical_item_variant_cold_query() -> TestResult {
    with_archive(|archive| {
        let selection = Selection::configured()?;
        let input = dataforge(archive)?;
        let selected = query(archive, &input, &selection)?;
        report("cold", &selected)
    })
}

#[test]
#[ignore = "real Archive: compare one physical Item variant with its exact Normal scope"]
fn installed_canonical_item_variant_matches_exact_scoped_normal_producer() -> TestResult {
    with_archive(|archive| {
        let selection = Selection::configured()?;
        let input = dataforge(archive)?;
        let expected = query(archive, &input, &selection)?;
        let filter = literal_filter(&input, selection.owner)?;
        let normal =
            variant_parity::scoped_normal(archive, &input, &filter, TABLE, &expected.origins)?;
        let Some([row]) = normal
            .pointer(TABLE)
            .and_then(Value::as_array)
            .map(Vec::as_slice)
        else {
            return Err("the exact Normal Item scope must publish only the selected owner".into());
        };
        if row != &expected.row
            || variant_parity::selected_set(&normal, row)? != Some(&expected.set)
        {
            return Err("the exact Normal and SDK Item row or variant evidence differs".into());
        }
        expected.origins.verify(&normal)?;
        report("parity", &expected)
    })
}

fn query(
    archive: &Archive,
    input: &DataForgeEntry,
    selection: &Selection,
) -> TestResult<SelectedItem> {
    let guids = selection.authenticate(input)?;
    let selected = with_items_from(
        archive,
        input,
        CatalogRecordSelection::RecordIndex(selection.owner),
        None,
        |snapshot| take_snapshot(archive, selection, &guids, &snapshot),
    )
    .map_err(|_| "the canonical Item query did not complete")??;
    Ok(selected)
}

fn take_snapshot(
    archive: &Archive,
    selection: &Selection,
    guids: &OriginalGuids,
    snapshot: &ItemSnapshot<'_>,
) -> TestResult<SelectedItem> {
    if snapshot.build() != archive.build()
        || !complete(snapshot.coverage())
        || !snapshot.requested_languages().is_empty()
        || !snapshot.available_languages().is_empty()
        || !snapshot.localization_notes().is_empty()
        || !snapshot.public_names().is_empty()
        || !snapshot.game_texts().is_empty()
    {
        return Err(
            "canonical Item requires complete same-Build coverage and no presentation text".into(),
        );
    }
    let [item] = snapshot.rows() else {
        return Err("exactly one nominated physical Item is required".into());
    };
    let header = item.source().source();
    if header.record_index().value() != Some(&selection.owner)
        || header.record_path().value().copied() != Some(selection.owner_path.as_str())
        || header.guid().value() != Some(&guids.owner)
    {
        return Err("the selected Item differs from the retained physical Record".into());
    }
    let proof = snapshot.provenance();
    let sets = proof.variant_source_use_sets();
    let id = item
        .variant_source_use_set_ref()
        .ok_or("the selected Item requires a shared variant cohort")?;
    if sets.iter_sets().len() != 1 {
        return Err("exactly one physical Item cohort is required".into());
    }
    let set = wire_row(sets.get(id).ok_or("the shared Item cohort is missing")?)?;
    let row = wire_row(item)?;
    let decision = item.variant_of();
    verify_target(item, proof.inventory(), selection, guids)?;
    let wire = row
        .get("variantOf")
        .ok_or("missing Item variant wire field")?;
    let origin = decision.origin();
    let encryption = serde_json::to_value(origin.contains_encrypted_source())?;
    if decision.state() != SourceValueState::Present
        || !origin.source_set_complete()
        || wire.get("state").and_then(Value::as_str) != Some("present")
        || wire.get("sourceSetComplete").and_then(Value::as_bool) != Some(true)
        || wire.get("containsEncryptedSource") != Some(&encryption)
        || wire.get("sourceUseSetRef").and_then(Value::as_u64) != Some(u64::from(id))
        || !wire
            .get("failedSourceUseRefs")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    {
        return Err("the canonical Item variant source health differs across API and wire".into());
    }
    variant_parity::verify_expanded_origin(wire, Some(&set), origin)?;
    let mut origins = Origins::default();
    origins.extend(proof.inventory(), slice::from_ref(&row))?;
    origins.extend(proof.inventory(), slice::from_ref(&set))?;
    Ok(SelectedItem { row, set, origins })
}

fn verify_target(
    item: &Item<'_>,
    inventory: &SourceInventory,
    selection: &Selection,
    guids: &OriginalGuids,
) -> TestResult {
    let target = item
        .variant_of()
        .value()
        .ok_or("a present original Item variant target is required")?;
    if target.target_record_index().value() != Some(&selection.base)
        || target.guid().value() != Some(&guids.base)
        || !item.variant_failed_source_uses().is_empty()
    {
        return Err("the selected Item does not resolve the exact external base".into());
    }
    verify_target_header(
        inventory,
        target.target_record_index().origin(),
        selection.base,
        "recordIndex",
        SourceUseRole::Selector,
    )?;
    verify_target_header(
        inventory,
        target.guid().origin(),
        selection.base,
        "guid",
        SourceUseRole::Payload,
    )?;
    Ok(())
}

fn verify_target_header(
    inventory: &SourceInventory,
    origin: &FieldOrigin,
    index: u32,
    field: &str,
    role: SourceUseRole,
) -> TestResult {
    let [reference] = origin.source_uses_refs() else {
        return Err("an Item base header must carry one immediate original use".into());
    };
    let used = inventory
        .iter_uses()
        .find(|used| used.id() == reference)
        .ok_or("an Item base header use is absent from its inventory")?;
    if !origin.source_set_complete()
        || used.role() != role
        || !matches!(
            used.location(),
            SourceLocation::Subresource { namespace, indices, field: original }
                if namespace == "dataforgeRecordHeader"
                    && indices.as_slice() == [u64::from(index)]
                    && original == field
        )
    {
        return Err("an Item base header use differs from its physical DCB coordinate".into());
    }
    Ok(())
}

fn report(phase: &str, item: &SelectedItem) -> TestResult {
    let wire = serde_json::to_vec(&(&item.row, &item.set))?;
    let digest = Checksum::Sha256(Sha256::digest(wire).into()).hex();
    print_line(&format!(
        "Canonical Item {phase} rows=1 variant=present digest={digest}"
    ));
    Ok(())
}
