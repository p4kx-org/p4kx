// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::catalog_queries::crafting::{BlueprintPoolSnapshot, BlueprintRewardSelection};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, Guid};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::super::{BLUEPRINT_POOL_WEIGHTS, BLUEPRINT_POOLS, BLUEPRINT_REWARDS, Route, TestResult};

#[path = "../../src/blueprint_reward_query_fixture.rs"]
mod fixture;

pub(super) fn read() -> TestResult<[Route; 3]> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("sdk_blueprint_pools.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::database(false).build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let [pools, weights] = pool_routes(&parsed)?;
    Ok([pools, weights, reward_route(&parsed)?])
}

fn pool_routes(parsed: &ParsedCatalogQueries<'_>) -> TestResult<[Route; 2]> {
    let selection = CatalogRecordSelection::RecordIndex(fixture::ALIAS);
    let (routes, guid) = parsed.with_blueprint_pools(selection, None, |snapshot| -> TestResult<([Route; 2], Guid)> {
        verify_pools(&snapshot, &[fixture::ALIAS])?;
        let [row] = snapshot.rows() else {
            return Err("expected the selected physical pool".into());
        };
        let guid = row.source().guid().value().copied().ok_or("original pool GUID")?;
        let definitions = Route::queried_rows(
            BLUEPRINT_POOLS,
            snapshot.rows(),
            "all, GUID and physical selectors exercised; complete installed producer parity open",
        )?;
        let weights = Route::queried_rows(
            BLUEPRINT_POOL_WEIGHTS,
            snapshot.weights(),
            "physical projection and selected installed witness available; complete installed producer parity open",
        )?;
        Ok(([definitions, weights], guid))
    })??;
    parsed.with_blueprint_pools(CatalogRecordSelection::Guid(guid), None, |snapshot| {
        verify_pools(&snapshot, &[0, fixture::ALIAS])
    })??;
    parsed.with_blueprint_pools(CatalogRecordSelection::All, None, |snapshot| {
        verify_pools(&snapshot, &[0, fixture::ALIAS, 2])
    })??;
    Ok(routes)
}

fn verify_pools(snapshot: &BlueprintPoolSnapshot<'_>, expected: &[u32]) -> TestResult {
    if snapshot.rows().len() != expected.len() || snapshot.weights().len() != expected.len() {
        return Err(
            "pool definition and weight counts must match the selected physical owners".into(),
        );
    }
    for ((definition, weight), index) in
        snapshot.rows().iter().zip(snapshot.weights()).zip(expected)
    {
        if definition.source().record_index().value() != Some(index)
            || weight.pool().record_index().value() != Some(index)
        {
            return Err(
                "pool definition and weight must retain the same ordered physical owner".into(),
            );
        }
    }
    Ok(())
}

fn reward_route(parsed: &ParsedCatalogQueries<'_>) -> TestResult<Route> {
    let source = BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::RecordIndex(
        fixture::SOURCE,
    ));
    let (route, guid) = parsed.with_blueprint_reward_sources(source, None, |snapshot| -> TestResult<(Route, Guid)> {
        if snapshot.source_records_read() != 1 || snapshot.rows().iter().any(|row| {
            row.source().record_index().value() != Some(&fixture::SOURCE)
        }) {
            return Err("a physical source query must isolate the requested reward owner".into());
        }
        let guid = snapshot.rows().first()
            .and_then(|row| row.source().guid().value().copied())
            .ok_or("original reward owner GUID")?;
        let route = Route::queried_rows(
            BLUEPRINT_REWARDS,
            snapshot.rows(),
            "owner and inverse target scopes exercised; complete installed producer and dependency parity open",
        )?;
        Ok((route, guid))
    })??;
    let owners = BlueprintRewardSelection::SourceOwners(CatalogRecordSelection::Guid(guid));
    parsed.with_blueprint_reward_sources(owners, None, |snapshot| -> TestResult {
        let indices: BTreeSet<_> = snapshot
            .rows()
            .iter()
            .filter_map(|row| row.source().record_index().value().copied())
            .collect();
        if indices != BTreeSet::from([fixture::SOURCE, fixture::SOURCE_ALIAS]) {
            return Err("the reward owner GUID query must retain both physical aliases".into());
        }
        Ok(())
    })??;
    verify_inverse(parsed, &route)?;
    Ok(route)
}

fn verify_inverse(parsed: &ParsedCatalogQueries<'_>, route: &Route) -> TestResult {
    let target =
        BlueprintRewardSelection::TargetPools(CatalogRecordSelection::RecordIndex(fixture::ALIAS));
    parsed.with_blueprint_reward_sources(target, None, |snapshot| -> TestResult {
        if snapshot.rows().iter().any(|row| {
            row.pool().value().and_then(|pool| pool.resolved_index()) != Some(fixture::ALIAS)
        }) {
            return Err("an inverse query must select only the requested physical pool".into());
        }
        let inverse = Route::queried_rows(BLUEPRINT_REWARDS, snapshot.rows(), route.remaining)?;
        if inverse.schema != route.schema {
            return Err("source and inverse queries must expose the same reward schema".into());
        }
        Ok(())
    })?
}
