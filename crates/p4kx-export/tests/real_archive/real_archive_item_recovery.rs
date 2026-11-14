// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, RecordTree, TreeBuilder, Value,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value as Json;

#[path = "real_archive_item_recovery_legacy.rs"]
mod legacy;
#[path = "real_archive_item_recovery_policy.rs"]
mod policy;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const TYPE_NAME: &str = "ItemRecoveryConfigurationParams";
const POLICIES: &str = "libs/foundry/records/entitlementpolicies/";
const NESTED: [&str; 3] = ["cooldownParams", "deliveryParams", "repairParams"];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn item_recovery_cooldown_delivery_and_repair_match_original_source_fields() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open selected Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read selected DataForge")?;
    let filter = Filter::new(&format!("{POLICIES}*"));
    let options = CatalogOptions {
        filter: Some(&filter),
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "selected recovery catalog must publish")?;
    let catalog: Json = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    let forge = entry.open().map_err(|_| "open selected DataForge")?;
    let original = select(&forge, &watch, &filter)?;
    check(&original, &catalog)?;
    policy::check(&original, &catalog)?;
    watch
        .check()
        .map_err(|_| "Archive changed during source comparison")?;
    Ok(())
}

fn select<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    filter: &Filter,
) -> TestResult<RecordTree<'source>> {
    let mut trees = TreeBuilder::new();
    let mut best: Option<RecordTree<'_>> = None;
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during source selection")?;
        let is_policy = record.record_path().is_ok_and(|path| filter.matches(path))
            && record.type_name().is_ok_and(|name| name == TYPE_NAME);
        if is_policy {
            let tree = trees
                .build(&record)
                .map_err(|_| "read selected recovery Record")?;
            if best
                .as_ref()
                .is_none_or(|best| tree.record_path() < best.record_path())
            {
                best = Some(tree);
            }
        }
    }
    best.ok_or_else(|| "a positive recovery source selection is required".into())
}

fn check(original: &RecordTree<'_>, catalog: &Json) -> TestResult {
    let rows = catalog
        .pointer("/configurations/itemRecoveryEconomy/rows")
        .and_then(Json::as_array)
        .ok_or("canonical recovery economy")?;
    let [row] = rows.as_slice() else {
        return Err("exactly one selected economy".into());
    };
    if row
        .pointer("/source/recordIndex/value")
        .and_then(Json::as_u64)
        != Some(u64::from(original.record_index()))
    {
        return Err("selected recovery row has the wrong physical owner".into());
    }
    let Some(Value::Struct(economy)) = original.field("economyParams") else {
        return Err("selected policy must have readable economy parameters".into());
    };
    let payload = row
        .pointer("/economyParams/value")
        .ok_or("readable recovery economy")?;
    legacy::check(economy, payload, catalog, original.record_index())?;
    let mut checked = 0;
    for block in NESTED {
        let exported = policy::member(payload, block)?;
        match economy.field(block) {
            None if policy::text(exported, "/state") == Some("absent") => {}
            Some(Value::Null) if policy::text(exported, "/state") == Some("null") => {}
            Some(Value::Struct(original))
                if policy::text(exported, "/state") == Some("present") =>
            {
                if original.fields().is_empty() {
                    return Err("populated recovery block must have source fields".into());
                }
                let value = policy::member(exported, "value")?;
                for field in original.fields() {
                    let Value::Single(number) = field.value() else {
                        return Err("nested recovery field must be an authored Single".into());
                    };
                    let cell = policy::member(value, field.name())?;
                    let bits = format!("{:08x}", number.to_bits());
                    if policy::text(cell, "/state") != Some("present")
                        || policy::text(cell, "/value/rawBits") != Some(bits.as_str())
                    {
                        return Err("nested recovery field differs from the original bits".into());
                    }
                }
                checked += 1;
            }
            _ => return Err("recovery block source state does not match export".into()),
        }
    }
    if std::env::var_os("P4KX_EXPECT_NESTED_RECOVERY").is_some() && checked != NESTED.len() {
        return Err("the selected build did not exercise all three nested recovery blocks".into());
    }
    Ok(())
}
