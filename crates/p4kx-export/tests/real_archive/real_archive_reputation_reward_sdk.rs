// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;
use std::cmp::Ordering;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ReputationRewardAmountDefinition, ReputationRewardAmountSnapshot,
    with_reputation_reward_amounts,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, FieldOrigin, InputScope,
    PhysicalSourceKind, SourceEncryption, SourceInventory, SourceLocation, SourceUseRole,
    TreeBuilder,
};

use crate::real_archive_catalog_table_parity::{
    Origins, Table, TestResult, capture_rows, first_difference, verify_tables,
};
use crate::real_archive_reputation_sandbox;
use crate::real_archive_verification::print_line;

const TABLE: &str = "/definitions/reputationRewardAmounts";
const FAMILY: &str = "SReputationRewardAmount";
const BUILD: &str = "P4KX_REPUTATION_REWARD_BUILD";
const COUNTS: &str = "P4KX_REPUTATION_REWARD_SIGNED_COUNTS";

#[test]
#[ignore = "installed source: configure the Archive, Build change and independent signed census"]
fn installed_reputation_reward_sdk_matches_original_signed_records() -> TestResult {
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configure the matching Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "configure the original Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the original Archive")?;
    verify_build(&archive)?;
    let expected = expected_counts()?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read the original DataForge Entry")?;
    let source = entry.open().map_err(|_| "parse the original DataForge")?;
    let queries = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = queries.parsed(None)?;
    let counts =
        parsed.with_reputation_reward_amounts(CatalogRecordSelection::All, None, |sdk| {
            verify_snapshot(&archive, &sdk)?;
            let observed = verify_original(&source, &entry, &sdk, &watch)?;
            if observed != expected {
                return Err("signed SDK census differs from the selected Build receipt".into());
            }
            for row in &sdk.rows {
                let index = *row
                    .source()
                    .record_index()
                    .value()
                    .ok_or("original reward SDK index")?;
                parsed.with_reputation_reward_amounts(
                    CatalogRecordSelection::RecordIndex(index),
                    None,
                    |point| verify_point(&archive, row, &point),
                )??;
            }
            Ok::<_, Box<dyn std::error::Error>>(observed)
        })??;
    watch.check()?;
    print_line(&format!(
        "original signed reward SDK: rows={} negative={} zero={} positive={}",
        counts.total(),
        counts.negative,
        counts.zero,
        counts.positive,
    ));
    Ok(())
}

#[test]
#[ignore = "complete same-Build Catalog: configure the Archive, census and measurement directory"]
fn installed_reputation_reward_sdk_matches_the_same_complete_catalog() -> TestResult {
    let parity = verify_tables([TABLE], capture_sdk)?;
    let [rows] = parity.rows;
    print_line(&format!(
        "complete reputation reward SDK parity: rows={rows} sourceUses={} physicalSources={}",
        parity.source_uses, parity.physical_sources,
    ));
    Ok(())
}

fn capture_sdk(archive: &Archive, origins: &mut Origins) -> TestResult<[Table; 1]> {
    verify_build(archive)?;
    let expected = expected_counts()?;
    with_reputation_reward_amounts(archive, CatalogRecordSelection::All, None, |sdk| {
        verify_snapshot(archive, &sdk)?;
        let mut counts = Signed::default();
        for row in &sdk.rows {
            counts.record(*row.reputation_amount().value().ok_or("signed SDK amount")?);
        }
        if counts != expected {
            return Err(
                "SDK does not match the independently measured same-Build signed census".into(),
            );
        }
        let table = capture_rows(
            sdk.availability,
            &sdk.rows,
            &sdk.source_inventory,
            origins,
            |row| {
                row.pointer("/recordIndex/value")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| "original physical reward index".into())
            },
        )?;
        Ok([table])
    })?
}

fn verify_original(
    forge: &DataForge<'_>,
    entry: &DataForgeEntry,
    sdk: &ReputationRewardAmountSnapshot<'_>,
    watch: &ArchiveWatch,
) -> TestResult<Signed> {
    let DataForgeOrigin::Archive(input) = entry.origin() else {
        return Err("the reward census requires the original physical DataForge Entry".into());
    };
    let mut owners = original_owners(forge, watch)?;
    owners.sort_by_cached_key(|record| (record.guid().to_string(), record.index()));
    if owners.is_empty() || owners.len() != sdk.rows.len() {
        return Err("the SDK must return every original physical reward Record".into());
    }
    let dataforge_entry = input.entry().index();
    let marker = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let mut trees = TreeBuilder::new();
    let mut signed = Signed::default();
    for (owner, row) in owners.into_iter().zip(&sdk.rows) {
        watch.poll()?;
        let tree = trees
            .build(&owner)
            .map_err(|_| "read an original reward Record")?;
        let index = owner.index();
        let header = row.source();
        if header.record_index().value() != Some(&index)
            || header.guid().value() != Some(&owner.guid())
            || header.record_path().value() != Some(&owner.record_path()?)
            || header.source_type().value() != Some(&FAMILY)
        {
            return Err("SDK reward owner identity or canonical Record order differs".into());
        }
        real_archive_reputation_sandbox::check_reward(
            &serde_json::to_value(row)?,
            tree.field("reputationAmount"),
            tree.field("editorName"),
        )?;
        let amount = *row
            .reputation_amount()
            .value()
            .ok_or("original signed SDK amount")?;
        signed.record(amount);
        for field in [row.reputation_amount().origin(), row.editor_name().origin()] {
            verify_original_payload(field, index, dataforge_entry, marker, &sdk.source_inventory)?;
        }
    }
    Ok(signed)
}

fn original_owners<'forge>(
    forge: &'forge DataForge<'_>,
    watch: &ArchiveWatch,
) -> TestResult<Vec<p4kx_extraction::Record<'forge>>> {
    let mut owners = Vec::new();
    for record in forge.records() {
        watch.poll()?;
        if record
            .type_name()
            .map_err(|_| "classify original reward Record family")?
            == FAMILY
        {
            owners.push(record);
        }
    }
    Ok(owners)
}

fn verify_original_payload(
    field: &FieldOrigin,
    owner: u32,
    dataforge_entry: usize,
    marker: Option<bool>,
    inventory: &SourceInventory,
) -> TestResult {
    let observed = inventory.iter_uses().any(|used| {
        if used.role() != SourceUseRole::Payload
            || used.scope() != InputScope::Payload
            || !field.source_uses_refs().contains(used.id())
        {
            return false;
        }
        let record = match used.location() {
            SourceLocation::DataForge { record_index, .. } => *record_index == owner,
            SourceLocation::Subresource {
                namespace, indices, ..
            } => namespace == "dataforgeValue" && indices.first() == Some(&u64::from(owner)),
            _ => false,
        };
        record
            && inventory.iter_sources().any(|source| {
                source.id() == used.source()
                    && source.ancestors().is_empty()
                    && match source.kind() {
                        PhysicalSourceKind::ArchiveEntry(entry) => {
                            entry.entry_index() == dataforge_entry
                        }
                        PhysicalSourceKind::ArchiveEntryProof(entry) => {
                            usize::try_from(entry.entry_index().get()) == Ok(dataforge_entry)
                        }
                        _ => false,
                    }
            })
    });
    if !observed || !field.source_set_complete() || field.contains_encrypted_source() != marker {
        return Err("a reward datum does not cite its actual selected DataForge owner".into());
    }
    Ok(())
}

fn verify_point(
    archive: &Archive,
    expected: &ReputationRewardAmountDefinition<'_>,
    point: &ReputationRewardAmountSnapshot<'_>,
) -> TestResult {
    verify_snapshot(archive, point)?;
    let [actual] = point.rows.as_slice() else {
        return Err("exact physical SDK selection must return one original reward row".into());
    };
    let expected = serde_json::to_value(expected)?;
    let actual = serde_json::to_value(actual)?;
    if let Some(location) = first_difference(&expected, &actual) {
        return Err(format!("reward All and RecordIndex selections differ at {location}").into());
    }
    let mut origins = Origins::default();
    origins.extend(&point.source_inventory, std::slice::from_ref(&actual))
}

fn verify_snapshot(archive: &Archive, sdk: &ReputationRewardAmountSnapshot<'_>) -> TestResult {
    if sdk.build.as_ref() != archive.build()
        || !matches!(sdk.availability, DatasetAvailability::Available)
        || !sdk.coverage.unreadable_headers.is_empty()
        || !sdk.coverage.unreadable_bodies.is_empty()
        || !sdk.coverage.incomplete_fields.is_empty()
        || sdk.coverage.skipped_leftovers != 0
        || sdk.rows.is_empty()
    {
        return Err("expected complete original reward coverage for the selected Build".into());
    }
    Ok(())
}

fn verify_build(archive: &Archive) -> TestResult {
    let expected = std::env::var(BUILD)?;
    if archive
        .build()
        .is_none_or(|build| build.change_number() != expected)
    {
        return Err("Archive Build does not match the independent signed census receipt".into());
    }
    Ok(())
}

#[derive(Default, PartialEq, Eq)]
struct Signed {
    negative: usize,
    zero: usize,
    positive: usize,
}

impl Signed {
    fn record(&mut self, amount: i32) {
        match amount.cmp(&0) {
            Ordering::Less => self.negative += 1,
            Ordering::Equal => self.zero += 1,
            Ordering::Greater => self.positive += 1,
        }
    }

    const fn total(&self) -> usize {
        self.negative + self.zero + self.positive
    }
}

fn expected_counts() -> TestResult<Signed> {
    let values: Vec<usize> = std::env::var(COUNTS)?
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    let [negative, zero, positive] = values.as_slice() else {
        return Err("expected census counts in negative, zero and positive order".into());
    };
    let counts = Signed {
        negative: *negative,
        zero: *zero,
        positive: *positive,
    };
    if counts.negative == 0 || counts.zero == 0 || counts.positive == 0 {
        return Err("independent census must exercise each original Int32 sign".into());
    }
    Ok(counts)
}
