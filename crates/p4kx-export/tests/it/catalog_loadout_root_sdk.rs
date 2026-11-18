// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::{LoadoutOwnerKind, read_loadouts};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestPort, TestRecord, TestStruct, entity_schema};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

const ITEM_GUID: u128 = 0xf0;
const SHIP_GUID: u128 = 0x10;

#[test]
fn loadout_root_identity_survives_item_staging_and_matches_the_sdk() -> TestResult {
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
        assert_eq!(selected.coverage.skipped_leftovers, 1);
        for row in &selected.rows {
            let index = row.source_record_index();
            let guid = row.source_entity_guid();
            let kind = row.source_entity_kind();
            assert_eq!(index.value(), Some(&row.record_index()));
            assert_eq!(guid.value(), Some(&row.entity_guid()));
            assert_eq!(kind.value(), Some(&row.entity_kind()));
            for marker in [index.origin(), guid.origin(), kind.origin()] {
                assert_eq!(marker.contains_encrypted_source(), Some(encrypted));
            }
        }
        let rows = serde_json::to_value(&selected.rows)?;
        assert_eq!(
            catalog["loadouts"], rows,
            "the Item spool preserves typed owner values"
        );
        verify_identity(&catalog, encrypted)?;
        let wire = json!({"loadouts": rows, "provenance": selected.source_inventory});
        verify_identity(&wire, encrypted)?;
    }
    Ok(())
}

#[test]
fn loadout_owner_kind_retains_the_known_absent_name_selector() -> TestResult {
    let database = entity_schema(DataForgeBuilder::new()).record(
        TestEntity::new(
            SHIP_GUID,
            "FixtureUnnamed",
            "libs/foundry/records/entities/groundvehicles/fixture/owner",
        )
        .record()
        .without_name(),
    );
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let selected = read_loadouts(&archive, CatalogRecordSelection::RecordIndex(0), None)?;
    assert_eq!(selected.coverage.skipped_leftovers, 0);
    let output = json!({"loadouts": selected.rows, "provenance": selected.source_inventory});
    let [row] = proof::rows(&output, "/loadouts")? else {
        return Err("the admitted original owner with no stored name".into());
    };
    verify_row(&output, row, 0, SHIP_GUID, "ship", false)?;
    proof::closure(&output)
}

fn verify_identity(output: &Value, encrypted: bool) -> TestResult {
    let [ship, item] = proof::rows(output, "/loadouts")? else {
        return Err("exactly two physical original Loadout rows".into());
    };
    verify_row(output, ship, 1, SHIP_GUID, "ship", encrypted)?;
    verify_row(output, item, 0, ITEM_GUID, "item", encrypted)?;
    proof::closure(output)
}

fn verify_row(
    output: &Value,
    row: &Value,
    index: u32,
    guid: u128,
    kind: &str,
    encrypted: bool,
) -> TestResult {
    let index_cell = proof::present(row, "sourceRecordIndex", &json!(index), Some(encrypted))?;
    let guid_cell = proof::present(
        row,
        "entityGuid",
        &json!(common::guid(guid)),
        Some(encrypted),
    )?;
    let kind_cell = proof::present(row, "entityKind", &json!(kind), Some(encrypted))?;
    let coordinate = index.to_string();
    verify_uses(
        output,
        index_cell,
        &coordinate,
        &[("recordIndex", "selector")],
    )?;
    verify_uses(output, guid_cell, &coordinate, &[("guid", "payload")])?;
    verify_uses(
        output,
        kind_cell,
        &coordinate,
        &[
            ("fileName", "conversion"),
            ("fileName", "selector"),
            ("recordName", "selector"),
            ("sourceType", "selector"),
        ],
    )
}

fn verify_uses(output: &Value, cell: &Value, index: &str, expected: &[(&str, &str)]) -> TestResult {
    let uses = proof::uses(output, cell)?;
    let mut actual = Vec::with_capacity(uses.len());
    for usage in uses {
        assert_eq!(
            proof::at(usage, "/location/namespace")?,
            "dataforgeRecordHeader",
            "original DataForge header namespace"
        );
        assert_eq!(
            proof::at(usage, "/location/indices/0")?,
            index,
            "physical record index"
        );
        let field = proof::at(usage, "/location/field")?
            .as_str()
            .ok_or("original header field")?;
        let role = proof::at(usage, "/role")?
            .as_str()
            .ok_or("original source use role")?;
        actual.push((field, role));
    }
    actual.sort_unstable();
    assert_eq!(
        actual, expected,
        "only the field's actual original header inputs"
    );
    Ok(())
}

fn root_database() -> DataForgeBuilder {
    entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(
                ITEM_GUID,
                "FixtureLoadoutItem",
                "libs/foundry/records/entities/scitem/fixture/root_item.xml",
            )
            .port(TestPort::new("fixture_port"))
            .record(),
        )
        .record(
            TestEntity::new(
                SHIP_GUID,
                "FixtureContestant",
                r"LIBS\Foundry/Records\Entities/SPACESHIPS/fixture/root_ship.XML",
            )
            .record(),
        )
        .record(
            TestEntity::new(
                0x30,
                "Fixture_LOADOUT_TEST",
                "libs/foundry/records/entities/scitem/fixture/with_ports.xml",
            )
            .port(TestPort::new("fixture_port"))
            .record(),
        )
        .record(
            TestEntity::new(
                0x40,
                "FixtureItemWithoutPorts",
                "libs/foundry/records/entities/legacyitems/fixture/no_ports.xml",
            )
            .record(),
        )
        .structure(TestStruct::new("entityclassdefinition"))
        .record(TestRecord::new(
            0x50,
            "entityclassdefinition",
            "entityclassdefinition.FixtureWrongType",
            "libs/foundry/records/entities/spaceships/fixture/wrong_type.xml",
        ))
}
