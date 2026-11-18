// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::{LoadoutOwnerKind, read_loadouts};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestPort, entity_schema};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn loadout_root_coordinates_survive_item_staging_and_match_the_sdk() -> TestResult {
    let database = root_database();
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        let selected = read_loadouts(&archive, CatalogRecordSelection::All, None)?;
        let [ship, item] = selected.rows.as_slice() else {
            return Err("one retained Ship and one staged Item".into());
        };
        assert!(matches!(ship.entity_kind(), LoadoutOwnerKind::Ship));
        assert!(matches!(item.entity_kind(), LoadoutOwnerKind::Item));
        assert_eq!(ship.record_index(), 1);
        assert_eq!(item.record_index(), 0);
        for row in &selected.rows {
            let source = row.source_record_index();
            assert_eq!(source.value(), Some(&row.record_index()));
            assert_eq!(source.origin().contains_encrypted_source(), Some(encrypted));
        }
        let rows = serde_json::to_value(&selected.rows)?;
        assert_eq!(
            catalog["loadouts"], rows,
            "the Item spool preserves the same typed root wire"
        );
        verify_identity(&catalog, encrypted)?;
        let wire = json!({"loadouts": rows, "provenance": selected.source_inventory});
        verify_identity(&wire, encrypted)?;
    }
    Ok(())
}

fn verify_identity(output: &Value, encrypted: bool) -> TestResult {
    let [ship, item] = proof::rows(output, "/loadouts")? else {
        return Err("exactly two physical original Loadout rows".into());
    };
    for (row, index, coordinate) in [(ship, 1, "1"), (item, 0, "0")] {
        let value = proof::present(row, "sourceRecordIndex", &json!(index), Some(encrypted))?;
        let uses = proof::uses(output, value)?;
        let [usage] = uses.as_slice() else {
            return Err("the original physical coordinate supplies one immediate use".into());
        };
        assert_eq!(
            proof::at(usage, "/role")?,
            "selector",
            "header selection role"
        );
        assert_eq!(
            proof::at(usage, "/location/namespace")?,
            "dataforgeRecordHeader",
            "the coordinate is in the original DataForge header"
        );
        assert_eq!(
            proof::at(usage, "/location/field")?,
            "recordIndex",
            "physical index field"
        );
        assert_eq!(
            proof::at(usage, "/location/indices/0")?,
            coordinate,
            "original physical position"
        );
    }
    proof::closure(output)
}

fn root_database() -> DataForgeBuilder {
    entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(
                0xf0,
                "FixtureLoadoutItem",
                "libs/foundry/records/entities/scitem/fixture/root_item.xml",
            )
            .port(TestPort::new("fixture_port"))
            .record(),
        )
        .record(
            TestEntity::new(
                0x10,
                "FixtureLoadoutShip",
                "libs/foundry/records/entities/spaceships/fixture/root_ship.xml",
            )
            .record(),
        )
}
