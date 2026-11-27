// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::Entities;
use p4kx_extraction::{
    FieldOrigin, InputScope, PhysicalSourceRef, SourceEncryptionState, SourceInput,
    SourceInventory, SourceLocation, SourceUseRole, Value,
};
use p4kx_testkit::{
    DataForgeBuilder, TestCapacity, TestEntity, TestEntry, TestLoadout, closed_container_record,
    entity_schema,
};

use super::super::ShipPool;
use super::super::cargo_record_comparison_tests::read_catalog_index;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::ship_pool_test_input;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::record_header_field::RecordHeaderField;

const ROOT_FILE: &str = "synthetic/cargo-admission.xml";
const DOCKING_FILE: &str = "synthetic/cargo-docking.xml";
const DOCKED_FILE: &str = "synthetic/cargo-dock-body.xml";
const INVENTORY: u128 = 1;
const GRID_RECORD: u32 = 1;
const CLASS_TEMPLATE: u32 = 2;
const PATH_TEMPLATE: u32 = 3;
const DOCKED: u32 = 4;
const DESCENDANT: u32 = 5;
const SHIP: u32 = 6;

#[test]
fn cargo_admission_retains_only_the_original_checks_that_ran() -> TestResult {
    ship_pool_test_input::with_input(&database(), entries(), |archive, files, forge, input| {
        let pool = ShipPool::new(Arc::new(Entities::new(forge)), files);
        let record = forge.record(SHIP).ok_or("physical cargo Ship")?;
        let plain = pool
            .read(&record, &record.tree()?)
            .map_err(|failure| failure.reason)?;
        let observed = read_catalog_index(&pool, input, SHIP)?;
        let observed_base = observed
            .read
            .map_err(|failure| failure.reason().to_owned())?;
        assert_eq!(plain.base.cargo, observed_base.base.cargo);
        let cargo = observed
            .aggregates
            .ok_or("resolved cargo projection")?
            .cargo;
        assert_eq!(cargo.scu().value(), Some(&1.25));
        assert_eq!(cargo.grids_with_issue().value(), Some(&0));
        let [mount] = cargo.mounts() else {
            return Err("exactly one mount passes the admission checks".into());
        };
        assert_eq!(
            mount.attachment().target_record_index().value(),
            Some(&GRID_RECORD)
        );

        let docked = archive
            .entries()
            .iter()
            .find(|entry| entry.path() == format!("Data/{DOCKED_FILE}"))
            .ok_or("docked vehicle's physical XML Entry")?;
        assert!(
            !observed
                .ledger
                .entries
                .iter()
                .any(|row| row.entry_index == Some(docked.index())),
            "the docked vehicle's descendants were not read"
        );

        let docking = archive
            .entries()
            .iter()
            .find(|entry| entry.path() == format!("Data/{DOCKING_FILE}"))
            .and_then(|entry| archive.entry_origin(entry))
            .ok_or("original docking Loadout admission")?;
        let sources = &input.origins().uses;
        let docking = sources.observe(SourceInput::Archive(&docking), InputScope::Payload)?;
        let inventory = sources.freeze_with(|| Ok::<_, CatalogError>(()))?;
        for origin in [cargo.scu().origin(), cargo.grids_with_issue().origin()] {
            assert_eq!(origin.contains_encrypted_source(), Some(true));
            assert!(!origin.source_set_complete());
            assert!(
                cites_encrypted_docking(&inventory, origin, docking.source_ref()),
                "the actual encrypted docking selector controls grid admission"
            );
        }
        assert_admission_inputs(
            input,
            &inventory,
            [cargo.scu().origin(), cargo.grids_with_issue().origin()],
        )
    })
}

#[derive(Clone, Copy)]
enum Checks {
    Unreached,
    Type,
    Class,
    Path,
}

fn assert_admission_inputs(
    input: &OriginalCatalogInput<'_, '_>,
    inventory: &SourceInventory,
    origins: [&FieldOrigin; 2],
) -> TestResult {
    for (index, checks) in [
        (GRID_RECORD, Checks::Path),
        (CLASS_TEMPLATE, Checks::Class),
        (PATH_TEMPLATE, Checks::Path),
        (DOCKED, Checks::Type),
        (DESCENDANT, Checks::Unreached),
        (SHIP, Checks::Unreached),
    ] {
        let original = original_type(input, index)?;
        let name = RecordHeaderField::RecordName.location(index);
        let filename = RecordHeaderField::FileName.location(index);
        for origin in origins {
            let selected = |location: &SourceLocation, role| {
                inventory.iter_uses().any(|used| {
                    used.location() == location
                        && used.role() == role
                        && origin.source_uses_refs().contains(used.id())
                })
            };
            assert_eq!(
                selected(&original, SourceUseRole::Selector),
                !matches!(checks, Checks::Unreached),
                "the original Type is selected for reached attachments at Record {index}"
            );
            assert_eq!(
                selected(&name, SourceUseRole::Selector),
                matches!(checks, Checks::Class | Checks::Path),
                "the RecordName selector follows the CargoGrid Type check at Record {index}"
            );
            for role in [SourceUseRole::Selector, SourceUseRole::Conversion] {
                assert_eq!(
                    selected(&filename, role),
                    matches!(checks, Checks::Path),
                    "the filename is converted and selected only after the class check at Record {index}"
                );
            }
        }
    }
    Ok(())
}

fn cites_encrypted_docking(
    inventory: &SourceInventory,
    origin: &FieldOrigin,
    docking: &PhysicalSourceRef,
) -> bool {
    inventory.iter_uses().any(|used| {
        used.source() == docking
            && used.role() == SourceUseRole::Selector
            && used.effective_encryption() == SourceEncryptionState::Encrypted
            && matches!(used.location(), SourceLocation::Subresource { field, .. } if field == "@itemName")
            && origin.source_uses_refs().contains(used.id())
    })
}

fn original_type(input: &OriginalCatalogInput<'_, '_>, index: u32) -> TestResult<SourceLocation> {
    let record = input.forge().record(index).ok_or("physical type Record")?;
    let tree = record.tree()?;
    let scope = input.record_scope(&record, &tree)?;
    let fields = OriginalFlowFields::read(&scope, None)?;
    let mut types = fields.facts().iter().filter(|fact| {
        fact.field().name() == "Type" && matches!(fact.value(), Value::String(_) | Value::Enum(_))
    });
    let original = types.next().ok_or("original AttachDef.Type operand")?;
    assert!(
        types.next().is_none(),
        "one original Type in the fixture Record"
    );
    Ok(original.location(OriginalValuePart::Value))
}

fn entries() -> Vec<TestEntry> {
    let xml = |path, body: &str| TestEntry::stored(format!("Data/{path}"), body.as_bytes());
    vec![
        xml(ROOT_FILE, r#"<Loadout><Loadouts><File loadout="synthetic/cargo-docking.xml"/></Loadouts><Items><Item portName="grid" itemName="Grid"/><Item portName="classTemplate" itemName="Class_TEMPLATE"/><Item portName="pathTemplate" itemName="PathOnly"/></Items></Loadout>"#),
        xml(DOCKING_FILE, r#"<Loadout><Items><Item portName="dock" itemName="Docked"/></Items></Loadout>"#).encrypted(),
        xml(DOCKED_FILE, r#"<Loadout><Items><Item portName="descendant" itemName="Descendant"/></Items></Loadout>"#).encrypted(),
    ]
}

fn database() -> Vec<u8> {
    let entity = |guid, name, file| {
        TestEntity::new(
            guid,
            name,
            format!("libs/foundry/records/entities/spaceships/{file}.xml"),
        )
    };
    let grid = |guid, name, file| {
        entity(guid, name, file)
            .attach_type("cArGoGrId")
            .container(INVENTORY)
            .record()
    };
    let ship = |guid, name, file, loadout: &str| {
        entity(guid, name, file)
            .attach_type("NOITEM_Vehicle")
            .loadout(TestLoadout::Xml(loadout.to_owned()))
            .record()
    };
    entity_schema(DataForgeBuilder::new().version(8))
        .record(closed_container_record(
            INVENTORY,
            "admission",
            TestCapacity::Centi(125),
        ))
        .record(grid(2, "Grid", "grid"))
        .record(grid(3, "Class_TEMPLATE", "ordinary_path"))
        .record(grid(4, "PathOnly", "path_CARGOGRID_TEMPLATE"))
        .record(ship(5, "Docked", "docked", DOCKED_FILE))
        .record(grid(6, "Descendant", "descendant"))
        .record(ship(7, "CargoShip", "cargo_ship", ROOT_FILE))
        .build()
}
