// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::slice;

use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry,
    TestPort, entity_schema, write_build_manifest,
};
use serde_json::Value;

use super::loadout_wire_row;
use crate::real_archive_catalog_table_parity::{Origins, TestResult, fingerprint, wire_row};
use crate::real_archive_scoped_equipment_sdk::equipment_input;

#[test]
fn direct_loadout_wire_preserves_nested_full_proof() -> TestResult {
    let (_folder, archive) = nested_ship_archive()?;
    let session =
        equipment_input::full_source_session(&archive, CatalogLanguageSelection::Selected(&[]))?;
    let summary = session.visit_loadouts(
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let loadout = snapshot
                .row
                .as_ref()
                .ok_or("the sample ship has a Loadout")?;
            let direct = loadout_wire_row(loadout)?;
            let roundtrip = wire_row(loadout)?;
            assert_eq!(
                direct, roundtrip,
                "the direct conversion preserves the complete JSON row"
            );
            let gun = direct
                .pointer("/ports/0/ports/0")
                .ok_or("nested gun port")?;
            assert_eq!(
                gun.get("itemClassName").and_then(Value::as_str),
                Some("ACME_Gun"),
                "the sample resolves the nested gun attachment",
            );
            assert!(
                gun.pointer("/name/sourceUsesRefs")
                    .and_then(Value::as_array)
                    .is_some_and(|refs| !refs.is_empty()),
                "the nested gun port carries original source references",
            );
            let mut origins = Origins::default();
            origins.extend(&snapshot.source_inventory, slice::from_ref(&direct))?;
            let (uses, sources) = origins.contributing_counts();
            assert!(
                uses > 0 && sources > 0,
                "the Full SDK inventory resolves the row's original proof"
            );
            assert_eq!(
                fingerprint(direct)?,
                fingerprint(roundtrip)?,
                "the comparison receives the same canonical digest",
            );
            Ok(())
        },
    )?;
    assert_eq!(
        (summary.owner_count, summary.row_count),
        (1, 1),
        "one sample ship emits one Loadout"
    );
    Ok(())
}

fn nested_ship_archive() -> TestResult<(tempfile::TempDir, Archive)> {
    const MOUNT: u128 = 0x20;
    const GUN: u128 = 0x30;
    let ship = TestEntity::new(
        0x10,
        "ACME_Ship",
        "libs/foundry/records/entities/spaceships/acme_ship.xml",
    )
    .port(TestPort::new("nose").sizes(2, 3))
    .loadout(TestLoadout::Manual(vec![
        TestLoadoutEntry::reference("nose", MOUNT).loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::reference("mount_gun", GUN),
        ])),
    ]));
    let mount = TestEntity::new(
        MOUNT,
        "ACME_Mount",
        "libs/foundry/records/entities/scitem/ships/acme_mount.xml",
    )
    .port(TestPort::new("mount_gun").sizes(2, 3));
    let gun = TestEntity::new(
        GUN,
        "ACME_Gun",
        "libs/foundry/records/entities/scitem/ships/acme_gun.xml",
    );
    let forge = entity_schema(DataForgeBuilder::new())
        .record(ship.record())
        .record(mount.record())
        .record(gun.record());

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    write_build_manifest(folder.path(), "fixture", "1.2.3", "456", "nested-loadout")?;
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", forge.build()))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    Ok((folder, archive))
}
