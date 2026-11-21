// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    WeaponAction, WeaponRecoilSnapshot, with_weapon_recoil_definitions_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::{DataForgeEntry, Filter};
use p4kx_testkit::TestRecord;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

use super::catalog_weapon_operating_fixture as fixture;

#[path = "catalog_actor_procedural_recoil_sdk.rs"]
mod actor;

const WEAPON: u32 = 2;
const RECOIL: u32 = 1;
const RECOIL_GUID: u128 = 0x9af4;
const RECOIL_TYPE: &str = "WeaponProceduralRecoilConfigDef";
const DATASET: &str = "/definitions/weaponActionRecoilDefinitions/rows";

#[test]
fn weapon_recoil_sdk_joins_original_actions_and_matches_the_canonical_rows() -> TestResult {
    let database = fixture::database();
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let filter = Filter::new("*/entities/scitem/*");
        let options = CatalogOptions {
            filter: Some(&filter),
            ..common::options()
        };
        let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
        let session =
            CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        let target =
            parsed.with_weapon_sources(WEAPON, None, |snapshot| -> TestResult<u32> {
                let [owner] = snapshot.rows() else {
                    return Err("one physical weapon owner".into());
                };
                let [weapon] = owner.weapon_actions.as_slice() else {
                    return Err("one original weapon component".into());
                };
                let [first, second, aim] = weapon.action_nodes() else {
                    return Err("two linked fire actions and one independent aim action".into());
                };
                let linked = |action: &WeaponAction<'_>| {
                    action
                        .recoil()
                        .target()
                        .value()
                        .and_then(|reference| reference.resolved_index())
                };
                assert_eq!(linked(first), Some(RECOIL), "the first fire slot");
                assert_eq!(linked(second), Some(RECOIL), "the second fire slot");
                assert_eq!(
                    linked(aim),
                    None,
                    "the separate aim slot has no recoil link"
                );
                assert_eq!(
                    first
                        .recoil()
                        .interval()
                        .origin()
                        .contains_encrypted_source(),
                    Some(encrypted)
                );
                linked(first).ok_or_else(|| "one exact physical recoil target".into())
            })??;

        parsed.with_weapon_recoil_definitions(
            CatalogRecordSelection::RecordIndex(target),
            None,
            |snapshot| -> TestResult {
                let [row] = snapshot.rows() else {
                    return Err("one physically selected recoil definition".into());
                };
                assert_eq!(
                    row.source().record_index().value(),
                    Some(&target),
                    "the linked physical definition"
                );
                assert_eq!(
                    row.aim().origin().contains_encrypted_source(),
                    Some(encrypted)
                );
                assert_eq!(snapshot.coverage().unreadable_bodies, [] as [u32; 0]);
                assert_eq!(snapshot.coverage().incomplete_fields, [] as [u32; 0]);
                let scoped = json!({
                    "definitions": { "weaponActionRecoilDefinitions": { "rows": snapshot.rows() } },
                    "provenance": snapshot.source_inventory(),
                });
                assert_eq!(
                    proof::rows(&scoped, DATASET)?,
                    proof::rows(&catalog, DATASET)?
                );
                proof::closure(&scoped)
            },
        )??;
    }
    Ok(())
}

#[test]
fn indexed_weapon_recoil_sdk_leaves_unreadable_siblings_outside_scope() -> TestResult {
    const SECOND: u32 = 3;
    const UNREADABLE: u32 = 4;
    let database = fixture::database()
        .record(TestRecord::new(
            RECOIL_GUID,
            RECOIL_TYPE,
            "WeaponProceduralRecoilConfigDef.Second",
            "libs/foundry/records/fixture/action_recoil_second.xml",
        ))
        .record(
            TestRecord::new(
                RECOIL_GUID,
                RECOIL_TYPE,
                "WeaponProceduralRecoilConfigDef.Unreadable",
                "libs/foundry/records/fixture/action_recoil_unreadable.xml",
            )
            .with_instance_index(u16::MAX),
        );
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    with_weapon_recoil_definitions_from(
        &archive,
        &entry,
        CatalogRecordSelection::RecordIndex(SECOND),
        None,
        |snapshot| assert_recoil_selection(&snapshot, &[SECOND], &[]),
    )??;

    let guid = common::guid(RECOIL_GUID).parse()?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    for selection in [
        CatalogRecordSelection::Guid(guid),
        CatalogRecordSelection::All,
    ] {
        parsed.with_weapon_recoil_definitions(selection, None, |snapshot| {
            assert_recoil_selection(&snapshot, &[RECOIL, SECOND], &[UNREADABLE])
        })??;
    }
    Ok(())
}

fn assert_recoil_selection(
    snapshot: &WeaponRecoilSnapshot<'_>,
    expected_indices: &[u32],
    expected_unreadable: &[u32],
) -> TestResult {
    let indices = snapshot
        .rows()
        .iter()
        .map(|row| {
            row.source()
                .record_index()
                .value()
                .copied()
                .ok_or("a recoil definition has no physical Record index")
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        indices, expected_indices,
        "readable selected physical Records"
    );
    assert_eq!(
        snapshot.coverage().unreadable_bodies,
        expected_unreadable,
        "unreadable selected physical Records"
    );
    Ok(())
}
