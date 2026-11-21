// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, ArchiveWatch, DataForge, Record};
use serde::Serialize;

use crate::catalog_queries::crafting::{
    BlueprintRewardSelection, BlueprintRewardSnapshot, BlueprintRewardSource,
};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};

use super::selection::{eligible_pool, original_pool_record};
use super::verify::{Graph, Proof};
use super::{TestResult, binding, open_archive, receipt, scope};

#[path = "installed_sdk_reward_original.rs"]
mod original;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    schema_version: u8,
    build_sha256: String,
    archive_bytes: u64,
    #[serde(flatten)]
    observation: Observation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Observation {
    available_source_records: usize,
    source_record_attempts: usize,
    same_guid_pool_candidates: usize,
    unreadable_same_guid_headers: usize,
    pool_record_attempts: usize,
    selected_pool_body_unreadable: bool,
    returned_rows: usize,
    sampled_source_bodies: usize,
    sampled_source_slots: usize,
    physical_sources: usize,
    retained_source_uses: usize,
    source_coverage: [usize; 4],
    pool_coverage: [usize; 4],
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_blueprint_reward_sdk_matches_original_target_and_sampled_sources() -> TestResult {
    let destination = receipt::destination()?;
    let archive = open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    let session = CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)
        .map_err(|_| "installed reward SDK session open failed")?;
    let parsed = session
        .parsed(None)
        .map_err(|_| "installed reward DataForge open failed")?;
    binding(&archive, &session, &parsed)?;
    let target = original_pool_record(&parsed.forge)?;
    let proof = Proof::read(session.dataforge_origin())?;
    let selection =
        BlueprintRewardSelection::TargetPools(CatalogRecordSelection::RecordIndex(target.index()));
    let observation = parsed
        .with_blueprint_reward_sources(selection, None, |snapshot| {
            check_snapshot(&archive, &parsed.forge, &target, &proof, &snapshot)
        })
        .map_err(|_| "installed reward SDK inverse query failed")??;
    watch
        .check()
        .map_err(|_| "installed Archive changed during the reward-source witness")?;
    receipt::publish(
        &destination,
        &Receipt {
            schema_version: 1,
            build_sha256: receipt::build_digest(&archive)?,
            archive_bytes: archive.file_len(),
            observation,
        },
    )
}

fn check_snapshot(
    archive: &Archive,
    forge: &DataForge<'_>,
    target: &Record<'_>,
    proof: &Proof,
    snapshot: &BlueprintRewardSnapshot<'_>,
) -> TestResult<Observation> {
    if snapshot.build() != archive.build()
        || !matches!(snapshot.availability(), DatasetAvailability::Available)
        || !matches!(snapshot.selection(),
            BlueprintRewardSelection::TargetPools(CatalogRecordSelection::RecordIndex(index))
                if index == target.index())
    {
        return Err(
            "inverse result Build, family or selection differs from the original target".into(),
        );
    }
    let pool_record_attempts = snapshot.pool_records_read();
    let pools = checked_pool_inputs(forge, target, pool_record_attempts)?;
    let mut admitted_pools = pools.admitted;
    let same_guid_pool_candidates = admitted_pools.len();
    let available_source_records = forge.records().len();
    if snapshot.source_records_read() != available_source_records {
        return Err("inverse query did not report its full original source survey".into());
    }

    let owners = checked_owners(snapshot.rows(), target)?;
    let inventory = snapshot.source_inventory();
    let leaf = proof.mapped_leaf(inventory)?;
    let graph = Graph::read(inventory)?;
    graph.physical_closure(archive, proof)?;
    admitted_pools.extend(&owners);
    admitted_pools.sort_unstable();
    admitted_pools.dedup();
    scope::validate_record_scope(archive, forge, inventory, leaf.id(), &admitted_pools)?;
    let sample = original::sample(forge, snapshot.rows(), &owners, target, &graph)?;
    Ok(Observation {
        available_source_records,
        source_record_attempts: snapshot.source_records_read(),
        same_guid_pool_candidates,
        unreadable_same_guid_headers: pools.unreadable_headers,
        pool_record_attempts,
        selected_pool_body_unreadable: snapshot
            .pool_coverage()
            .unreadable_bodies
            .contains(&target.index()),
        returned_rows: snapshot.rows().len(),
        sampled_source_bodies: sample.bodies,
        sampled_source_slots: sample.slots,
        physical_sources: inventory.source_count(),
        retained_source_uses: inventory.use_count(),
        source_coverage: receipt::coverage(snapshot.source_coverage()),
        pool_coverage: receipt::coverage(snapshot.pool_coverage()),
    })
}

#[derive(Default)]
struct PoolInputs {
    admitted: Vec<u32>,
    unreadable_headers: usize,
}

fn checked_pool_inputs(
    forge: &DataForge<'_>,
    target: &Record<'_>,
    attempts: usize,
) -> TestResult<PoolInputs> {
    let mut pools = PoolInputs::default();
    for candidate in forge.records_by_guid(target.guid()) {
        match eligible_pool(&candidate) {
            Ok(true) => pools.admitted.push(candidate.index()),
            Ok(false) => {}
            Err(_) => pools.unreadable_headers += 1,
        }
    }
    if !pools.admitted.contains(&target.index()) || !(1..=pools.admitted.len()).contains(&attempts)
    {
        return Err("inverse query pool attempts escaped the selected nominal identity".into());
    }
    Ok(pools)
}

fn checked_owners(rows: &[BlueprintRewardSource<'_>], target: &Record<'_>) -> TestResult<Vec<u32>> {
    let type_name = target
        .type_name()
        .map_err(|_| "original target type unavailable")?;
    let path = target
        .record_path()
        .map_err(|_| "original target path unavailable")?;
    let mut owners = Vec::new();
    let mut previous = None;
    for row in rows {
        let owner = owner_index(row)?;
        let ordinal = row
            .source_ordinal()
            .value()
            .ok_or("inverse row source ordinal missing")?
            .get();
        let key = (owner, ordinal);
        if previous.is_some_and(|previous| previous >= key) {
            return Err("inverse rows do not retain strict physical occurrence order".into());
        }
        previous = Some(key);
        if owners.last() != Some(&owner) {
            owners.push(owner);
        }
        let pool = row
            .pool()
            .value()
            .ok_or("inverse row pool target missing")?;
        if pool.resolved_index() != Some(target.index())
            || pool.guid().value().copied() != Some(target.guid())
            || pool.target_type().value().copied() != Some(type_name)
            || pool.record_path().value().copied() != Some(path)
        {
            return Err("inverse row target differs from its selected physical Record".into());
        }
    }
    Ok(owners)
}

fn owner_index(row: &BlueprintRewardSource<'_>) -> TestResult<u32> {
    row.source()
        .record_index()
        .value()
        .copied()
        .ok_or_else(|| "inverse row owner missing".into())
}
