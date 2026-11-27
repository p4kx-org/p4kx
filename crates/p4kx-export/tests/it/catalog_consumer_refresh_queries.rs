// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::crafting::BlueprintRewardSelection;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection, ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, CancelToken, Guid, Language, SourceInventory, SourcedValue};

use crate::catalog_loadouts::catalog_item_alignment;
use crate::commodity_v3_fixture::{TestResult, guid};
use crate::crafting_detail_fixture::POOL;
use crate::sdk_full_proof as full_proof;

use super::cache::Dashboard;
use super::fixture;
use super::origins;

pub(super) fn collect(
    archive: &Archive,
    language: Language,
    cancel: Option<&CancelToken>,
    after_reward: impl FnOnce(),
) -> TestResult<Dashboard> {
    let session = full_proof::open(
        archive,
        CatalogLanguageSelection::Selected(&[language]),
        cancel,
    )?;
    let parsed = session.parsed(cancel)?;
    let (pool_index, encrypted) = read_reward(&parsed, cancel, after_reward)?;
    let blueprint = read_blueprint(&parsed, pool_index, encrypted, cancel)?;
    let location_disabled = read_location(&parsed, encrypted, cancel)?;
    let first_port = read_port(&parsed, encrypted, cancel)?;
    Ok(Dashboard {
        blueprint,
        location_disabled,
        first_port,
        reward_link_encrypted: encrypted,
    })
}

fn read_reward(
    parsed: &ParsedCatalogQueries<'_>,
    cancel: Option<&CancelToken>,
    after_reward: impl FnOnce(),
) -> TestResult<(u32, bool)> {
    let selection = BlueprintRewardSelection::SourceOwners(by_guid(fixture::REWARD_OWNER)?);
    parsed.with_blueprint_reward_sources(
        selection,
        cancel,
        |snapshot| -> TestResult<(u32, bool)> {
            require_complete(snapshot.source_coverage())?;
            require_complete(snapshot.pool_coverage())?;
            assert_eq!(
                snapshot.source_records_read(),
                1,
                "one requested original owner"
            );
            assert_eq!(
                snapshot.pool_records_read(),
                1,
                "one demanded physical pool"
            );
            let [row] = snapshot.rows() else {
                return Err("one authored reward occurrence".into());
            };
            assert_eq!(
                row.source().guid().value(),
                Some(&guid(fixture::REWARD_OWNER).parse::<Guid>()?),
                "the reward retains its requested source owner"
            );
            let evidence = check_source(row.pool(), snapshot.source_inventory())?;
            let pool = row.pool().value().ok_or("authored reward pool")?;
            assert_eq!(
                pool.guid().value(),
                Some(&guid(POOL).parse::<Guid>()?),
                "the authored pool target"
            );
            after_reward();
            Ok((
                pool.resolved_index().ok_or("physical reward pool")?,
                evidence,
            ))
        },
    )?
}

fn read_blueprint(
    parsed: &ParsedCatalogQueries<'_>,
    pool_index: u32,
    encrypted: bool,
    cancel: Option<&CancelToken>,
) -> TestResult<Guid> {
    let (index, guid) = read_pool_blueprint(parsed, pool_index, encrypted, cancel)?;
    parsed.with_blueprints(
        CatalogRecordSelection::RecordIndex(index),
        cancel,
        |snapshot| {
            require_complete(snapshot.coverage())?;
            let dependencies = snapshot.dependency_coverage();
            require_complete(dependencies.globals())?;
            require_complete(dependencies.dismantle())?;
            require_complete(dependencies.categories())?;
            let [row] = snapshot.definitions() else {
                return Err("one referenced physical Blueprint".into());
            };
            let header = row.header();
            assert_eq!(
                header.record_index().value(),
                Some(&index),
                "the selected physical Blueprint"
            );
            assert_eq!(
                header.guid().value(),
                Some(&guid),
                "the authored Blueprint identity"
            );
            assert_eq!(
                check_source(&header.guid(), snapshot.source_inventory())?,
                encrypted,
                "the original header or field retains its encryption state"
            );
            Ok(guid)
        },
    )?
}

fn read_pool_blueprint(
    parsed: &ParsedCatalogQueries<'_>,
    pool_index: u32,
    encrypted: bool,
    cancel: Option<&CancelToken>,
) -> TestResult<(u32, Guid)> {
    parsed.with_blueprint_pools(
        CatalogRecordSelection::RecordIndex(pool_index),
        cancel,
        |snapshot| -> TestResult<(u32, Guid)> {
            require_complete(snapshot.coverage())?;
            let [pool] = snapshot.rows() else {
                return Err("one physical pool definition".into());
            };
            assert_eq!(
                pool.source().record_index().value(),
                Some(&pool_index),
                "the selected physical pool"
            );
            let [first, _] = pool
                .blueprint_rewards()
                .value()
                .ok_or("authored slots")?
                .as_slice()
            else {
                return Err("two original reward slots in authored order".into());
            };
            let link = first
                .value()
                .ok_or("first authored reward slot")?
                .blueprint_record();
            assert_eq!(
                check_source(link, snapshot.source_inventory())?,
                encrypted,
                "the original Blueprint link"
            );
            let target = link.value().ok_or("authored Blueprint reference")?;
            let index = target
                .resolved_index()
                .ok_or("physical Blueprint reference")?;
            let guid = target
                .guid()
                .value()
                .copied()
                .ok_or("authored Blueprint GUID")?;
            Ok((index, guid))
        },
    )?
}

fn read_location(
    parsed: &ParsedCatalogQueries<'_>,
    encrypted: bool,
    cancel: Option<&CancelToken>,
) -> TestResult<bool> {
    parsed.with_mission_location_templates(by_guid(fixture::LOCATION)?, cancel, |snapshot| {
        require_complete(&snapshot.coverage)?;
        let [location] = snapshot.rows.as_slice() else {
            return Err("one requested mission location template".into());
        };
        assert_eq!(
            check_source(location.disabled(), &snapshot.source_inventory)?,
            encrypted,
            "the original location field retains its encryption state"
        );
        location
            .disabled()
            .value()
            .copied()
            .ok_or_else(|| "authored location status".into())
    })?
}

fn read_port(
    parsed: &ParsedCatalogQueries<'_>,
    encrypted: bool,
    cancel: Option<&CancelToken>,
) -> TestResult<String> {
    parsed.with_item_ports(
        by_guid(catalog_item_alignment::OWNER)?,
        cancel,
        |snapshot| {
            require_complete(&snapshot.coverage)?;
            let [owner] = snapshot.rows.as_slice() else {
                return Err("one selected port owner".into());
            };
            let [first, _] = owner
                .ports()
                .value()
                .ok_or("authored port list")?
                .as_slice()
            else {
                return Err("two original ports in authored order".into());
            };
            let port = first.value().ok_or("first physical port")?;
            assert_eq!(
                port.port_ordinal().value(),
                Some(&0),
                "the first original port"
            );
            assert_eq!(
                check_source(port.name(), &snapshot.source_inventory)?,
                encrypted,
                "the original header or field retains its encryption state"
            );
            let name = port.name().value().ok_or("authored port name")?;
            Ok((*name).to_owned())
        },
    )?
}

pub(super) fn by_guid(value: u128) -> TestResult<CatalogRecordSelection> {
    Ok(CatalogRecordSelection::Guid(guid(value).parse::<Guid>()?))
}

pub(super) fn require_complete(coverage: &CatalogRecordCoverage) -> TestResult {
    if coverage.unreadable_headers.is_empty()
        && coverage.unreadable_bodies.is_empty()
        && coverage.incomplete_fields.is_empty()
        && coverage.skipped_leftovers == 0
    {
        Ok(())
    } else {
        Err("incomplete source scope for the requested example fields".into())
    }
}

fn check_source<T>(cell: &SourcedValue<T>, inventory: &SourceInventory) -> TestResult<bool> {
    origins::check(cell, inventory)?.ok_or_else(|| "known original encryption marker".into())
}
