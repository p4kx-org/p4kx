// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read as _;
use std::sync::Arc;

use p4kx_assembly::Entities;
use p4kx_extraction::{
    Archive, DataForgeEntry, Entry, FieldOrigin, InputScope, PhysicalSourceRef,
    SourceEncryptionState, SourceInput, SourceInventory, SourceLocation, SourceUseRole, Value,
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
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::SourceDisposition;
use crate::ship_base::ShipCargoSource;

const ROOT: &str = "synthetic/cargo-root.xml";
const MERGE: &str = "synthetic/cargo-merge.xml";
const BASE: &str = "synthetic/cargo-base.xml";
const WINNER: &str = "synthetic/cargo-winner.xml";
const BYSTANDER: &str = "synthetic/cargo-bystander.xml";
const FIRST: u32 = 3;
const SECOND: u32 = 4;
const CONTROL: u32 = 5;

#[test]
fn nested_xml_replacement_cites_the_compared_ports_and_the_current_cargo_request() -> TestResult {
    ship_pool_test_input::with_input(&database(), entries(), |archive, files, forge, input| {
        let pool = ShipPool::new(Arc::new(Entities::new(forge)), files);
        let first = read_catalog_index(&pool, input, FIRST)?;
        let second = read_catalog_index(&pool, input, SECOND)?;
        let control = read_catalog_index(&pool, input, CONTROL)?;

        assert!(first.read.is_ok() && second.read.is_ok() && control.read.is_ok());
        for path in [BASE, BYSTANDER] {
            let entry = xml_entry(archive, path)?;
            assert!(
                first
                    .ledger
                    .entries
                    .iter()
                    .any(|row| row.entry_index == Some(entry.index())
                        && row.operation == EntryOperation::XmlLoadout
                        && row.disposition == SourceDisposition::Projected),
                "the encrypted XML is genuinely read and projected"
            );
        }
        assert!(
            second.ledger.entries.is_empty(),
            "the second Ship uses the actual cached XML projection"
        );
        let first = first.aggregates.ok_or("first original cargo resolution")?;
        let second = second
            .aggregates
            .ok_or("cached original cargo resolution")?;
        let control = control
            .aggregates
            .ok_or("unmasked original cargo resolution")?;
        check(archive, input, &first.cargo, &second.cargo, &control.cargo)
    })
}

fn check(
    archive: &Archive,
    input: &OriginalCatalogInput<'_, '_>,
    first: &ShipCargoSource,
    second: &ShipCargoSource,
    control: &ShipCargoSource,
) -> TestResult {
    check_values(first, second, control);

    let includes = [
        xml_selector(archive, input, ROOT, "loadout", MERGE)?,
        xml_selector(archive, input, MERGE, "loadout", BASE)?,
        xml_selector(archive, input, MERGE, "loadout", WINNER)?,
    ];
    let displaced = xml_selector(archive, input, BASE, "portName", "CargoPad")?;
    let winner = xml_selector(archive, input, WINNER, "portName", "cARGOpAD")?;
    let unrelated = [
        xml_selector(archive, input, ROOT, "loadout", BYSTANDER)?,
        xml_selector(archive, input, BYSTANDER, "portName", "Bystander")?,
        xml_selector(archive, input, BASE, "portName", "Spare")?,
    ];
    let first_caller = record_selector(archive, input, FIRST)?;
    let second_caller = record_selector(archive, input, SECOND)?;
    assert_ne!(first_caller.location, second_caller.location);
    let inventory = input
        .origins()
        .uses
        .freeze_with(|| Ok::<_, CatalogError>(()))?;
    for (cargo, caller, other_caller) in [
        (first, &first_caller, &second_caller),
        (second, &second_caller, &first_caller),
    ] {
        for origin in origins(cargo) {
            for operand in includes.iter().chain([&winner]) {
                assert!(
                    operand.contributes(
                        &inventory,
                        origin,
                        Some(SourceEncryptionState::Unencrypted)
                    ),
                    "cargo retains winning port and Include selector {:?}",
                    operand.location
                );
            }
            assert!(
                displaced.contributes(&inventory, origin, Some(SourceEncryptionState::Encrypted)),
                "cargo retains the exact encrypted displaced port selector"
            );
            assert!(
                caller.contributes(&inventory, origin, Some(SourceEncryptionState::Unencrypted)),
                "the Cargo comparison belongs to the current physical DCB caller"
            );
            assert_excluded(&inventory, origin, unrelated.iter().chain([other_caller]));
        }
    }
    for origin in origins(control) {
        let compared = includes
            .iter()
            .chain([&winner, &first_caller, &second_caller]);
        assert_excluded(&inventory, origin, compared);
    }
    Ok(())
}

fn assert_excluded<'selector>(
    inventory: &SourceInventory,
    origin: &FieldOrigin,
    excluded: impl IntoIterator<Item = &'selector Selector>,
) {
    for selector in excluded {
        assert!(
            !selector.contributes(inventory, origin, None),
            "an unused selector contributes to the cargo field: {:?}",
            selector.location
        );
    }
}

fn check_values(first: &ShipCargoSource, second: &ShipCargoSource, control: &ShipCargoSource) {
    for masked in [first, second] {
        assert_eq!(
            masked.scu().value(),
            Some(&0.0),
            "the winning port is empty"
        );
        assert_eq!(masked.grids_with_issue().value(), Some(&0));
        assert!(masked.mounts().is_empty());
        for origin in origins(masked) {
            assert_eq!(origin.contains_encrypted_source(), Some(true));
            assert!(!origin.source_set_complete());
        }
    }
    assert_eq!(
        control.scu().value(),
        Some(&1.25),
        "unmasked original cargo"
    );
    assert_eq!(control.mounts().len(), 1);
}

fn origins(cargo: &ShipCargoSource) -> [&FieldOrigin; 2] {
    [cargo.scu().origin(), cargo.grids_with_issue().origin()]
}

struct Selector {
    physical: PhysicalSourceRef,
    location: SourceLocation,
}

impl Selector {
    fn contributes(
        &self,
        inventory: &SourceInventory,
        field: &FieldOrigin,
        encryption: Option<SourceEncryptionState>,
    ) -> bool {
        inventory.iter_uses().any(|used| {
            used.source() == &self.physical
                && used.role() == SourceUseRole::Selector
                && used.location() == &self.location
                && field.source_uses_refs().contains(used.id())
                && encryption.is_none_or(|state| used.effective_encryption() == state)
        })
    }
}

fn xml_selector(
    archive: &Archive,
    input: &OriginalCatalogInput<'_, '_>,
    path: &str,
    attribute: &str,
    text: &str,
) -> TestResult<Selector> {
    let entry = xml_entry(archive, path)?;
    let mut bytes = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut bytes)?;
    let document = roxmltree::Document::parse(std::str::from_utf8(&bytes)?)?;
    let mut location = None;
    for node in document.descendants().filter(roxmltree::Node::is_element) {
        for (ordinal, field) in node.attributes().enumerate() {
            if field.name() != attribute || field.value() != text {
                continue;
            }
            assert!(location.is_none(), "one original XML attribute occurrence");
            let mut indices = vec![u64::from(node.id().get())];
            let namespace = if attribute == "portName" {
                indices.push(u64::try_from(ordinal)?);
                "xmlAttribute"
            } else {
                "xml"
            };
            location = Some(SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices,
                field: format!("@{attribute}"),
            });
        }
    }
    Ok(Selector {
        physical: source(archive, input, entry)?,
        location: location.ok_or("exact original XML field")?,
    })
}

fn record_selector(
    archive: &Archive,
    input: &OriginalCatalogInput<'_, '_>,
    index: u32,
) -> TestResult<Selector> {
    let record = input
        .forge()
        .record(index)
        .ok_or("original caller Record")?;
    let tree = record.tree()?;
    let scope = input.record_scope(&record, &tree)?;
    let fields = OriginalFlowFields::read(&scope, None)?;
    let mut candidates = fields.facts().iter().filter(|fact| {
        fact.field().name() == "loadoutPath"
            && matches!(fact.value(), Value::String(path) if *path == ROOT)
    });
    let original = candidates.next().ok_or("original root XML getter")?;
    assert!(
        candidates.next().is_none(),
        "one original Record loadoutPath"
    );
    let dcb = DataForgeEntry::locate(archive).ok_or("physical input DataForge")?;
    Ok(Selector {
        physical: source(archive, input, dcb)?,
        location: original.location(OriginalValuePart::Value),
    })
}

fn source(
    archive: &Archive,
    input: &OriginalCatalogInput<'_, '_>,
    entry: &Entry,
) -> TestResult<PhysicalSourceRef> {
    let admitted = archive
        .entry_origin(entry)
        .ok_or("original Archive Entry admission")?;
    let scope = input
        .origins()
        .uses
        .observe(SourceInput::Archive(&admitted), InputScope::Payload)?;
    Ok(scope.source_ref().clone())
}

fn xml_entry<'archive>(archive: &'archive Archive, path: &str) -> TestResult<&'archive Entry> {
    let path = format!("Data/{path}");
    Ok(archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or("physical fixture XML Entry")?)
}

fn entries() -> Vec<TestEntry> {
    let xml = |path, content: &str| TestEntry::stored(format!("Data/{path}"), content.as_bytes());
    vec![
        xml(ROOT, r#"<Loadout><Loadouts><File loadout="synthetic/cargo-merge.xml"/><File loadout="synthetic/cargo-bystander.xml"/></Loadouts></Loadout>"#),
        xml(MERGE, r#"<Loadout><Loadouts><File loadout="synthetic/cargo-base.xml"/><File loadout="synthetic/cargo-winner.xml"/></Loadouts></Loadout>"#),
        xml(BASE, r#"<Loadout><Items><Item portName="CargoPad" itemName="Fixture_CargoGrid"/><Item portName="Spare" itemName="Fixture_Inert"/></Items></Loadout>"#).encrypted(),
        xml(WINNER, r#"<Loadout><Items><Item portName="cARGOpAD"/></Items></Loadout>"#),
        xml(BYSTANDER, r#"<Loadout><Items><Item portName="Bystander"/></Items></Loadout>"#).encrypted(),
    ]
}

fn database() -> Vec<u8> {
    let entity = |guid, name| {
        TestEntity::new(
            guid,
            name,
            format!("libs/foundry/records/entities/spaceships/{name}.xml"),
        )
    };
    let ship = |guid, name, path: &str| {
        entity(guid, name)
            .attach_type("NOITEM_Vehicle")
            .loadout(TestLoadout::Xml(path.to_owned()))
            .record()
    };
    entity_schema(DataForgeBuilder::new().version(8))
        .record(closed_container_record(
            1,
            "cargo",
            TestCapacity::Centi(125),
        ))
        .record(
            entity(2, "Fixture_CargoGrid")
                .attach_type("CargoGrid")
                .container(1)
                .record(),
        )
        .record(entity(3, "Fixture_Inert").attach_type("WeaponGun").record())
        .record(ship(4, "Fixture_First", ROOT))
        .record(ship(5, "Fixture_Second", ROOT))
        .record(ship(6, "Fixture_Control", BASE))
        .build()
}
