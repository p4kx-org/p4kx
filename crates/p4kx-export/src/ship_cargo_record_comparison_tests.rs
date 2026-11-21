// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::Entities;
use p4kx_extraction::{SourceInventory, SourceLocation, SourceUseRole, Value};
use p4kx_testkit::{
    DataForgeBuilder, TestCapacity, TestEntity, TestLoadout, TestLoadoutEntry, TestPort,
    closed_container_record, entity_schema,
};

use super::ShipPool;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::ship_pool_test_input;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::export_diagnostic::ExportSourceRole;
use crate::loadout::ledger::ShipAggregateValues;
use crate::record_observation::RecordObservation;
use crate::ship_base::ShipCargoSource;
use crate::{CatalogError, Kind};

const INVENTORY: u128 = 1;
const GRID: u128 = 2;
const CARRIER: u128 = 3;
const INERT: u128 = 4;
const MASKED: u32 = 4;
const CONTROL: u32 = 5;

#[test]
fn only_actual_record_port_comparisons_contribute_to_the_current_cargo_aggregate() -> TestResult {
    ship_pool_test_input::with_input(&database(), Vec::new(), |_, files, forge, input| {
        let pool = ShipPool::new(Arc::new(Entities::new(forge)), files);
        let masked = read(&pool, input, MASKED)?;
        let control = read(&pool, input, CONTROL)?;
        assert_eq!(
            masked.cargo.scu().value(),
            Some(&0.0),
            "both original grid choices were masked"
        );
        assert!(
            masked.cargo.mounts().is_empty(),
            "a discarded grid contributes no mount"
        );
        assert_eq!(
            control.cargo.scu().value(),
            Some(&2.5),
            "the next physical Ship resolves both independent grid choices"
        );
        assert_eq!(
            control.cargo.mounts().len(),
            2,
            "cached specs do not retain prior resolution effects"
        );
        let inventory = input
            .origins()
            .uses
            .freeze_with(|| Ok::<_, CatalogError>(()))?;
        verify_comparisons(input, &inventory, &masked.cargo, &control.cargo)?;
        assert!(
            !masked.cargo.scu().origin().source_set_complete(),
            "the exact comparisons cannot certify every membership decision"
        );
        assert_eq!(
            masked.cargo.scu().origin().contains_encrypted_source(),
            None,
            "known plain selectors leave the incomplete encryption conclusion unknown"
        );
        Ok(())
    })
}

fn read<'input>(
    pool: &ShipPool<'input>,
    input: &'input OriginalCatalogInput<'input, 'input>,
    index: u32,
) -> Result<ShipAggregateValues, Box<dyn std::error::Error>> {
    let record = input
        .forge()
        .record(index)
        .ok_or("one actual synthetic Ship Record")?;
    let tree = record.tree()?;
    let source = crate::export_field_walk::inspect(
        &tree,
        &record,
        Some(Kind::Ships),
        ExportSourceRole::Selected,
        None,
    )?;
    let operands = crate::catalog_v3::authored_fields::ship_source_read::aggregate_for_test(
        &tree,
        input,
        &RecordObservation::capture(input, &record)?,
    )?;
    Ok(pool
        .read_catalog(&operands, &record, &tree, &source, input, None)?
        .aggregates
        .ok_or("a resolved Ship aggregate")?)
}

fn verify_comparisons(
    input: &OriginalCatalogInput<'_, '_>,
    inventory: &SourceInventory,
    masked: &ShipCargoSource,
    control: &ShipCargoSource,
) -> TestResult {
    for (record, literal) in [
        (MASKED, "Suppressed"),
        (MASKED, "SUPPRESSED"),
        (2, "pad"),
        (MASKED, "PAD"),
    ] {
        let location = original_name(input, record, literal)?;
        let selected = inventory
            .iter_uses()
            .find(|used| used.role() == SourceUseRole::Selector && used.location() == &location)
            .ok_or("the exact physical name comparison selector")?;
        assert!(
            masked
                .scu()
                .origin()
                .source_uses_refs()
                .contains(selected.id())
                && masked
                    .grids_with_issue()
                    .origin()
                    .source_uses_refs()
                    .contains(selected.id()),
            "both sourced cargo aggregates retain {literal} at its exact selector occurrence"
        );
        assert!(
            !control
                .scu()
                .origin()
                .source_uses_refs()
                .contains(selected.id()),
            "the next caller does not reuse the earlier cargo comparison"
        );
    }
    let untouched = original_name(input, MASKED, "bay")?;
    assert!(
        !inventory.iter_uses().any(|used| {
            used.location() == &untouched
                && masked.scu().origin().source_uses_refs().contains(used.id())
        }),
        "an unmatched original port does not enter the comparison source set"
    );
    Ok(())
}

fn original_name(
    input: &OriginalCatalogInput<'_, '_>,
    index: u32,
    literal: &str,
) -> Result<SourceLocation, Box<dyn std::error::Error>> {
    let record = input
        .forge()
        .record(index)
        .ok_or("actual original operand Record")?;
    let tree = record.tree()?;
    let scope = input.record_scope(&record, &tree)?;
    let fields = OriginalFlowFields::read(&scope, None)?;
    let mut matching = fields.facts().iter().filter(|fact| {
        matches!(fact.field().name(), "Name" | "itemPortName")
            && matches!(fact.value(), Value::String(value) if *value == literal)
    });
    let value = matching.next().ok_or("one original comparison operand")?;
    if matching.next().is_some() {
        return Err("the fixture provides a unique raw comparison name in the Record".into());
    }
    Ok(value.location(OriginalValuePart::Value))
}

fn database() -> Vec<u8> {
    let entity = |guid, class| {
        TestEntity::new(
            guid,
            class,
            format!("libs/foundry/records/entities/spaceships/{class}.xml"),
        )
    };
    let carrier = || TestLoadoutEntry::reference("bay", CARRIER);
    let host =
        |guid, class| entity(guid, class).port(TestPort::new("Suppressed").default_item(GRID));
    entity_schema(DataForgeBuilder::new().version(8))
        .record(closed_container_record(
            INVENTORY,
            "cargo",
            TestCapacity::Centi(125),
        ))
        .record(
            entity(GRID, "Grid")
                .attach_type("CargoGrid")
                .container(INVENTORY)
                .record(),
        )
        .record(
            entity(CARRIER, "Carrier")
                .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::reference(
                    "pad", GRID,
                )]))
                .record(),
        )
        .record(entity(INERT, "Inert").attach_type("WeaponGun").record())
        .record(
            host(5, "Masked")
                .loadout(TestLoadout::Manual(vec![
                    TestLoadoutEntry::empty("SUPPRESSED"),
                    carrier().loadout(TestLoadout::Manual(vec![TestLoadoutEntry::reference(
                        "PAD", INERT,
                    )])),
                ]))
                .record(),
        )
        .record(
            host(6, "Control")
                .loadout(TestLoadout::Manual(vec![carrier()]))
                .record(),
        )
        .build()
}
