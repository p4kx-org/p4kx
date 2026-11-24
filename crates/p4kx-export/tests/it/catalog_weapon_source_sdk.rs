// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::equipment::{
    AmmoBinding, WeaponActions, WeaponModeKind, WeaponSourceSnapshot, with_selected_weapon_sources,
    with_selected_weapon_sources_from, with_weapon_sources, with_weapon_sources_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordCoverage, CatalogRecordSelection,
    ResourceFloat32,
};
use p4kx_extraction::DataForgeEntry;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType};
use serde_json::{Value, json};

use super::fixture::{self, Case};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const WEAPON: u32 = 3;
const UNREADABLE: u32 = 4;
const WEAPON_ALIAS: u32 = 4;
const ACTIONS: &str = "/configurations/weaponActions/rows";
const CONTAINERS: &str = "/configurations/ammoContainers/rows";
const BINDINGS: &str = "/declarations/ammoBindings/rows";

#[test]
fn physical_weapon_rows_match_canonical_actions_containers_and_bindings() -> TestResult {
    let database = fixture::build(Case::Unique);
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let catalog = serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        with_weapon_sources(&archive, WEAPON, None, |snapshot| {
            verify_rows(&snapshot, &catalog, encrypted)
        })??;
    }
    Ok(())
}

#[test]
fn selected_weapon_sources_keep_duplicate_guid_owners_in_producer_order() -> TestResult {
    let database = fixture::build(Case::Unique).record(
        TestRecord::new(
            fixture::WEAPON,
            "EntityClassDefinition",
            "EntityClassDefinition.ProjectileAlias",
            "libs/foundry/records/entities/scitem/fixture/projectile_alias.xml",
        )
        .with_instance_index(0),
    );
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let guid = CatalogRecordSelection::Guid(common::guid(fixture::WEAPON).parse()?);
    with_selected_weapon_sources(&archive, CatalogRecordSelection::All, None, |snapshot| {
        verify_selected_rows(&snapshot, &catalog)
    })??;
    session.with_selected_weapon_sources(guid, None, |snapshot| {
        verify_selected_rows(&snapshot, &catalog)
    })??;
    let parsed = session.parsed(None)?;
    parsed.with_selected_weapon_sources(
        CatalogRecordSelection::RecordIndex(WEAPON_ALIAS),
        None,
        |snapshot| -> TestResult {
            let [owner] = snapshot.rows() else {
                return Err("one owner at the exact physical index".into());
            };
            let [action] = owner.weapon_actions.as_slice() else {
                return Err("one authored action row from the alias".into());
            };
            assert_eq!(
                action.owner().record_index().value(),
                Some(&WEAPON_ALIAS),
                "the physical selector retains the alias rather than the first GUID occurrence",
            );
            Ok(())
        },
    )??;
    Ok(())
}

fn verify_selected_rows(snapshot: &WeaponSourceSnapshot<'_>, catalog: &Value) -> TestResult {
    let mut record_indices = Vec::new();
    for owner in snapshot.rows() {
        let [action] = owner.weapon_actions.as_slice() else {
            return Err("each physical owner contributes one action row".into());
        };
        record_indices.push(action.owner().record_index().value().copied());
    }
    assert_eq!(
        record_indices,
        [Some(WEAPON), Some(WEAPON_ALIAS)],
        "both original occurrences are returned in producer order"
    );
    assert_eq!(
        snapshot.coverage(),
        &CatalogRecordCoverage::default(),
        "both potential owners are fully readable"
    );
    let owners = snapshot.rows();
    let actions: Vec<_> = owners
        .iter()
        .flat_map(|owner| &owner.weapon_actions)
        .collect();
    let containers: Vec<_> = owners
        .iter()
        .flat_map(|owner| &owner.ammo_containers)
        .collect();
    let bindings: Vec<_> = owners
        .iter()
        .flat_map(|owner| &owner.ammo_bindings)
        .collect();
    let scoped = json!({
        "configurations": {
            "weaponActions": { "rows": actions },
            "ammoContainers": { "rows": containers },
        },
        "declarations": { "ammoBindings": { "rows": bindings } },
        "provenance": snapshot.source_inventory(),
    });
    for path in [ACTIONS, CONTAINERS, BINDINGS] {
        assert_eq!(
            proof::rows(&scoped, path)?,
            proof::rows(catalog, path)?,
            "all owned rows and their origins agree at {path}"
        );
    }
    proof::closure(&scoped)
}

#[test]
fn weapon_sources_leave_unrelated_unreadable_items_outside_the_query() -> TestResult {
    let database = fixture::build(Case::Unique).record(
        TestRecord::new(
            0xfa11,
            "EntityClassDefinition",
            "EntityClassDefinition.FixtureUnreadableItem",
            "libs/foundry/records/entities/scitem/fixture/unreadable_item.xml",
        )
        .with_instance_index(u16::MAX),
    );
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let entry = DataForgeEntry::read(&archive)?;
    with_weapon_sources_from(&archive, &entry, WEAPON, None, |snapshot| {
        assert_eq!(
            snapshot.rows().len(),
            1,
            "only the requested weapon is projected"
        );
        assert!(
            snapshot.coverage().unreadable_bodies.is_empty(),
            "the unrelated Item body stays out of scope"
        );
    })?;
    with_weapon_sources_from(&archive, &entry, UNREADABLE, None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "the unreadable Item supplies no rows"
        );
        assert_eq!(
            snapshot.coverage().unreadable_bodies,
            [UNREADABLE],
            "selecting that physical Item exposes its failure"
        );
    })?;
    let guid = CatalogRecordSelection::Guid(common::guid(fixture::WEAPON).parse()?);
    let cases: [(CatalogRecordSelection, &[u32]); 2] =
        [(CatalogRecordSelection::All, &[UNREADABLE]), (guid, &[])];
    for (selection, unreadable) in cases {
        with_selected_weapon_sources_from(&archive, &entry, selection, None, |snapshot| {
            assert_eq!(
                snapshot.rows().len(),
                1,
                "readable owner under {selection:?}"
            );
            assert_eq!(
                snapshot.coverage().unreadable_bodies,
                unreadable,
                "unreadable Item coverage under {selection:?}"
            );
        })?;
    }
    Ok(())
}

#[test]
fn an_unreadable_component_retains_the_readable_weapon_and_incomplete_scope() -> TestResult {
    let database = fixture::build(Case::UnreadableComponent);
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    with_weapon_sources(&archive, WEAPON, None, |snapshot| -> TestResult {
        let [owner] = snapshot.rows() else {
            return Err("the readable weapon owner stays in the result".into());
        };
        assert_eq!(
            owner.weapon_actions.len(),
            1,
            "only the authored readable weapon"
        );
        assert_eq!(
            snapshot.coverage().incomplete_fields,
            [WEAPON],
            "the missing component kind cannot be confirmed absent"
        );
        Ok(())
    })?
}

#[test]
fn ambiguous_or_invalid_component_declarations_cannot_establish_absence() -> TestResult {
    let components = || TestType::StrongPointer("DataForgeComponentParams".to_owned());
    for (declarations, complete) in [
        (vec![components()], true),
        (vec![components(), components()], false),
        (vec![TestType::Int32], false),
    ] {
        let owner = declarations.into_iter().fold(
            TestStruct::new("EntityClassDefinition"),
            |owner, declaration| owner.array("Components", declaration),
        );
        let database = DataForgeBuilder::new()
            .version(8)
            .structure(TestStruct::new("DataForgeComponentParams"))
            .structure(owner)
            .record(TestRecord::new(
                1,
                "EntityClassDefinition",
                "EntityClassDefinition.EmptyFixtureItem",
                "libs/foundry/records/entities/scitem/fixture/empty_item.xml",
            ));
        let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
        with_weapon_sources(&archive, 0, None, |snapshot| {
            assert!(
                snapshot.rows().is_empty(),
                "there is no readable weapon row"
            );
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                if complete { vec![] } else { vec![0] },
                "the declared array must be unique and capable of storing components",
            );
        })?;
    }
    Ok(())
}

fn verify_rows(
    snapshot: &WeaponSourceSnapshot<'_>,
    catalog: &Value,
    encrypted: bool,
) -> TestResult {
    let [owner] = snapshot.rows() else {
        return Err("one physically selected weapon owner".into());
    };
    let [weapon] = owner.weapon_actions.as_slice() else {
        return Err("one canonical weapon component".into());
    };
    assert_eq!(
        weapon.owner().record_index().value(),
        Some(&WEAPON),
        "the authored selector retains the exact physical owner"
    );
    assert_eq!(
        weapon.fire_actions().origin().contains_encrypted_source(),
        Some(encrypted),
        "the authored slots retain their original source marker"
    );
    verify_typed_shot(weapon, encrypted)?;
    assert_ammo_slots(&owner.ammo_bindings)?;
    let scoped = json!({
        "configurations": {
            "weaponActions": { "rows": owner.weapon_actions },
            "ammoContainers": { "rows": owner.ammo_containers },
        },
        "declarations": { "ammoBindings": { "rows": owner.ammo_bindings } },
        "provenance": snapshot.source_inventory(),
    });
    for path in [ACTIONS, CONTAINERS, BINDINGS] {
        assert_eq!(
            proof::rows(&scoped, path)?,
            proof::rows(catalog, path)?,
            "original values and evidence references agree at {path}"
        );
    }
    proof::closure(&scoped)
}

fn verify_typed_shot(weapon: &WeaponActions<'_>, encrypted: bool) -> TestResult {
    let action = weapon
        .action_nodes()
        .first()
        .ok_or("at least one authored action")?;
    assert_eq!(
        action.mode_kind(),
        WeaponModeKind::SimpleShot,
        "the rapid action belongs to the nominal single-discharge family",
    );
    let shot = action
        .combat_inputs()
        .ok_or("an authored projectile launcher")?;
    assert_eq!(
        (
            shot.fire_rate.value(),
            shot.pellet_count.value(),
            shot.damage_multiplier
                .value()
                .and_then(ResourceFloat32::finite),
        ),
        (Some(&120), Some(&3), Some(0.5)),
        "the typed accessors retain the physical action's authored values",
    );
    assert_eq!(
        shot.fire_rate.origin().contains_encrypted_source(),
        Some(encrypted),
        "the typed rate retains immediate encryption provenance",
    );
    Ok(())
}

fn assert_ammo_slots(bindings: &[AmmoBinding<'_>]) -> TestResult {
    let [
        AmmoBinding::WeaponMagazineSlots { value: weapon },
        AmmoBinding::ContainerSlots { value: container },
    ] = bindings
    else {
        return Err(
            "the physical weapon and its own container retain both declared bindings".into(),
        );
    };
    assert_eq!(
        weapon.ignore_primary_magazine().value(),
        Some(&false),
        "the source ignore flag is retained"
    );
    assert_eq!(
        container
            .primary_ammo()
            .value()
            .and_then(|target| target.resolved_index()),
        Some(0),
        "primary ammunition retains its resolved physical Record index"
    );
    assert_eq!(
        container
            .secondary_ammo()
            .value()
            .and_then(|target| target.resolved_index()),
        Some(1),
        "secondary ammunition retains its separate physical Record index"
    );
    Ok(())
}
