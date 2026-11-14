// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::time::SystemTime;

use p4kx_export::{
    CatalogOptions, CatalogPublicationMode, Kind, build_catalog_from, is_kind_leftover,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, InputScope, Record,
    SourceEncryption, SourceInput, SourceScope, SourceUses, TreeBuilder,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use super::{TestResult, definition_identity, list, print_line};
use crate::catalog_v3_consumer;
use crate::real_archive_item_durability::{self as proof, Trace};

#[path = "real_archive_ammo_values.rs"]
mod values;

const KIND: Kind = Kind::AmmoDefinitions;
const SOURCE_TYPE: &str = "AmmoParams";
const SAMPLES_PER_SHAPE: usize = 2;

#[derive(Default)]
struct Counts {
    readable: usize,
    unreadable: usize,
    skipped: usize,
    sampled: usize,
    shapes: usize,
    scalars: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn ammunition_definitions_match_original_physical_records_and_sampled_variant_values() -> TestResult
{
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read configured DataForge")?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options).map_err(|_| "build Ammo catalog")?;
    let catalog: Value = serde_json::from_slice(built.json()).map_err(|_| "read catalog JSON")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    definition_identity::check_original_ammo(&catalog, &forge)
        .map_err(|_| "canonical Ammo headers disagree with original physical Records")?;
    let counts = compare(&forge, entry.origin(), &catalog, &watch)?;
    watch
        .check()
        .map_err(|_| "Archive changed during comparison")?;
    assert!(
        counts.readable > 0 && counts.sampled > 0 && counts.scalars > 0,
        "the configured Build must exercise readable original Ammo definitions"
    );
    print_line(&format!(
        "ammunition: {} readable Records, {} unreadable Records, {} policy skips, {} sampled Records, {} original variant combinations, {} scalar values checked",
        counts.readable,
        counts.unreadable,
        counts.skipped,
        counts.sampled,
        counts.shapes,
        counts.scalars,
    ));
    Ok(())
}

fn compare(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    catalog: &Value,
    watch: &ArchiveWatch,
) -> Result<Counts, Box<dyn std::error::Error>> {
    let mut rows = physical_rows(catalog)?;
    let mut failed = failed_reads(catalog)?;
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let (scope, encrypted) = dcb_scope(origin)?;
    let mut counts = Counts::default();
    let mut sample_counts = BTreeMap::new();
    let mut trees = TreeBuilder::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during source census")?;
        if record.type_name().ok() != Some(SOURCE_TYPE) {
            continue;
        }
        if leftover(&record)? {
            counts.skipped += 1;
            continue;
        }
        let row = rows.remove(&record.index());
        let Ok(tree) = trees.build(&record) else {
            if row.is_some() || !failed.remove(&record.index()) {
                return Err(
                    "the unreadable physical Ammo source has inconsistent read accounting".into(),
                );
            }
            counts.unreadable += 1;
            continue;
        };
        let row = row.ok_or("the readable physical Ammo source has no definition row")?;
        counts.readable += 1;
        let shape = values::Shape::read(&tree);
        let sampled = sample_counts.entry(shape).or_insert(0);
        if *sampled < SAMPLES_PER_SHAPE {
            let trace = Trace::of(&tree, &uses, &scope, encrypted);
            counts.scalars += values::compare(&trace, &tree, row)?;
            counts.sampled += 1;
            *sampled += 1;
        }
    }
    if !rows.is_empty() || !failed.is_empty() {
        return Err("the Ammo output contains unmatched physical source positions".into());
    }
    counts.shapes = sample_counts.len();
    check_counts(catalog, &counts)?;
    Ok(counts)
}

fn dcb_scope(
    origin: &DataForgeOrigin,
) -> Result<(SourceScope, Option<bool>), Box<dyn std::error::Error>> {
    let DataForgeOrigin::Archive(entry) = origin else {
        return Err("expected an observed Archive DataForge".into());
    };
    let sources = SourceUses::new();
    let scope = sources
        .observe(SourceInput::Archive(entry), InputScope::Payload)
        .map_err(|_| "observe actual DataForge origin")?;
    let encrypted = match origin.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    Ok((scope, encrypted))
}

fn leftover(record: &Record<'_>) -> Result<bool, Box<dyn std::error::Error>> {
    let path = record
        .record_path()
        .map_err(|_| "read original Ammo path metadata")?;
    let name = record
        .name()
        .map_err(|_| "read original Ammo name metadata")?;
    Ok(is_kind_leftover(KIND, path, name))
}

fn physical_rows(catalog: &Value) -> Result<BTreeMap<u32, &Value>, Box<dyn std::error::Error>> {
    let mut rows = BTreeMap::new();
    for row in list(catalog, KIND).map_err(|_| "read canonical Ammo definition rows")? {
        let carrier = row
            .get("recordIndex")
            .ok_or("physical Ammo position carrier")?;
        let index = catalog_v3_consumer::present(carrier)?
            .as_u64()
            .ok_or("physical Ammo position")?;
        if rows.insert(u32::try_from(index)?, row).is_some() {
            return Err("the same Ammo Record position is written twice".into());
        }
    }
    Ok(rows)
}

fn failed_reads(catalog: &Value) -> Result<BTreeSet<u32>, Box<dyn std::error::Error>> {
    let mut indices = BTreeSet::new();
    for row in proof::rows(catalog, "/exportHealth/failedRecords")? {
        if row.get("kind").and_then(Value::as_str) != Some(KIND.name())
            || row.get("role").and_then(Value::as_str) != Some("selected")
            || row.get("stage").and_then(Value::as_str) != Some("sourceRead")
        {
            continue;
        }
        let index = u32::try_from(proof::number(row, "/recordIndex/value")?)?;
        if !indices.insert(index) {
            return Err("one failed read per selected physical Ammo position".into());
        }
    }
    Ok(indices)
}

fn check_counts(catalog: &Value, counts: &Counts) -> TestResult {
    let health = proof::rows(catalog, "/exportHealth/kinds")?
        .iter()
        .find(|row| row.get("kind").and_then(Value::as_str) == Some(KIND.name()))
        .ok_or("Ammo source-health Kind")?;
    for (field, expected) in [
        (
            "/sourceRecords/selected",
            counts.readable + counts.unreadable + counts.skipped,
        ),
        ("/sourceRecords/readSuccessful", counts.readable),
        ("/sourceRecords/projected", counts.readable),
        ("/sourceRecords/intentionallySkipped", counts.skipped),
        ("/sourceRecords/omittedAfterReadFailure", counts.unreadable),
        ("/sourceRecords/omittedAfterProjectionFailure", 0),
        ("/writtenRows", counts.readable),
    ] {
        if proof::number(health, field)? != u64::try_from(expected)? {
            return Err(format!("the Ammo source census disagrees with {field}").into());
        }
    }
    Ok(())
}
