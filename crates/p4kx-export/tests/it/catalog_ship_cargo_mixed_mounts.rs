// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection, equipment};
use p4kx_extraction::{SourceEncryptionState, SourceInventory};
use p4kx_testkit::{DataForgeBuilder, TestCapacity, TestEntity, TestEntry, TestLoadout};
use serde_json::Value;

use super::fixture::{GRID, GRID_RECORD, INVENTORY, SHIP, item};
use crate::health_origin_fixture::{Fixture, Input, TestResult};

const HOST_FILE: &str = "Data/synthetic/cargo-host.xml";
const DOCKED_FILE: &str = "Data/synthetic/cargo-docked.xml";
const HOST_XML: &str = r#"<Loadout><Items><Item portName="grid" itemName="Fixture_WordGrid"/><Item portName="docked" itemName="Fixture_Docked"/></Items></Loadout>"#;
const DOCKED_XML: &str = r#"<Loadout><Items><Item portName="docked_grid" itemName="Fixture_WordGrid"/></Items></Loadout>"#;

#[test]
fn only_the_reached_encrypted_attachment_can_mark_the_plain_inventory_cargo() -> TestResult {
    let database = database();
    for selected_encrypted in [false, true] {
        let selected = TestEntry::stored(HOST_FILE, HOST_XML.as_bytes());
        let selected = if selected_encrypted {
            selected.encrypted()
        } else {
            selected
        };
        let fixtures = Fixture::open(
            &database,
            Input::Plain,
            vec![
                selected,
                TestEntry::stored(DOCKED_FILE, DOCKED_XML.as_bytes()).encrypted(),
            ],
        )?;
        let (_, catalog) = fixtures.diagnostic()?;
        let [projection] =
            crate::health_origin_assertions::rows(&catalog, "/projections/shipBaseFacts/rows")?
        else {
            return Err("one selected physical host Ship".into());
        };
        full_proof::from_entry(
            &fixtures.archive,
            &fixtures.entry,
            CatalogLanguageSelection::Selected(&[]),
            None,
        )?
        .with_ship_sources(CatalogRecordSelection::All, None, |snapshot| {
            assert_sdk_cargo(&snapshot, projection, selected_encrypted)
        })??;
        crate::health_origin_assertions::closure(&catalog)?;
    }
    Ok(())
}

fn assert_sdk_cargo(
    snapshot: &equipment::ShipSourceSnapshot<'_>,
    projection: &Value,
    selected_encrypted: bool,
) -> TestResult {
    let [ship] = snapshot.rows() else {
        return Err("one scoped physical Ship".into());
    };
    let cargo = ship.cargo();
    assert_eq!(
        cargo.scu().value(),
        Some(&1.25),
        "the docked descendant is excluded"
    );
    assert_eq!(
        cargo.grids_with_issue().value(),
        Some(&0),
        "exactly one readable grid"
    );
    assert_eq!(
        projection["cargo"],
        serde_json::to_value(cargo)?,
        "SDK and producer preserve the same cargo evidence"
    );
    let [grid] = cargo.mounts() else {
        return Err("one actual loadout mount".into());
    };
    assert_eq!(
        grid.attachment().target_record_index().value(),
        Some(&GRID_RECORD),
        "the selected physical CargoGrid"
    );
    assert_eq!(
        grid.capacity_scu().origin().contains_encrypted_source(),
        Some(false),
        "the amount comes from the plain original inventory"
    );
    assert_eq!(
        cargo.scu().origin().contains_encrypted_source(),
        selected_encrypted.then_some(true),
        "only the actually selected encrypted XML can prove an encrypted input"
    );
    assert_marker_uses(snapshot.source_inventory(), cargo, selected_encrypted);
    Ok(())
}

fn assert_marker_uses(
    inventory: &SourceInventory,
    cargo: &equipment::ShipCargoSource,
    selected_encrypted: bool,
) {
    let source_uses = cargo.scu().origin().source_uses_refs();
    let encrypted = inventory.iter_uses().any(|used| {
        source_uses.contains(used.id())
            && used.effective_encryption() == SourceEncryptionState::Encrypted
    });
    assert_eq!(
        encrypted, selected_encrypted,
        "only the reached encrypted selector contributes to the cargo total"
    );
}

fn database() -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8))
        .record(p4kx_testkit::closed_container_record(
            INVENTORY,
            "fixture_first",
            TestCapacity::Centi(125),
        ))
        .record(p4kx_testkit::closed_container_record(
            INVENTORY,
            "fixture_second",
            TestCapacity::Centi(975),
        ))
        .record(item(GRID, "Fixture_WordGrid").container(INVENTORY).record())
        .record(
            TestEntity::new(
                0x7a1a,
                "Fixture_Docked",
                "libs/foundry/records/entities/scitem/ships/fixture_docked.xml",
            )
            .attach_type("NOITEM_Vehicle")
            .loadout(TestLoadout::Xml("synthetic/cargo-docked.xml".to_owned()))
            .record(),
        )
        .record(
            TestEntity::new(
                SHIP,
                "Fixture_CargoShip",
                "libs/foundry/records/entities/spaceships/fixture_cargo_ship.xml",
            )
            .loadout(TestLoadout::Xml("synthetic/cargo-host.xml".to_owned()))
            .record(),
        )
        .build()
}
