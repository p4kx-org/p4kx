// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{LoadoutResolver, PortDefinitionPosition};
use p4kx_extraction::{DataForge, SourceUses, SourceValue};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestPort, TestStruct, entity_schema};

use super::super::{
    AlignmentAxis, AlignmentIndex, AlignmentOptions, ItemAlignmentRows, ItemPort, PortRestrictions,
};
use super::{CONTAINER, ComponentOwner, ItemPortContainer, ItemPortStaging};
use crate::CatalogError;
use crate::build::original_input::ship_pool_test_input;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::catalog_entity_configurations;
use crate::record_observation::RecordObservation;
use crate::texts::{TextSources, Texts};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn staged_ports_match_the_original_sorted_catalog_table_bytes() -> TestResult {
    let directory = tempfile::tempdir()?;
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let sources = SourceUses::new();
    let mut staged = ItemPortStaging::new(directory.path(), None);
    let mut original = Vec::new();
    let positions = [(0, 1, true), (0, 2, false), (0, 2, true), (2, 0, false)];
    for (record, component, present) in positions {
        staged.append(row(&sources, record, component, present))?;
        original.push(row(&sources, record, component, present));
    }
    original.sort_by_key(|row| row.owner.key());
    let typed = serde_json::to_vec(&CatalogTable::new(&forge, &[CONTAINER], original))?;
    let prepared = staged.finish(&forge)?;
    let slot = prepared
        .facts()
        .first()
        .and_then(|row| row.ports.first())
        .ok_or("retained nameless physical slot")?;
    assert_eq!(
        slot.port_ordinal.value(),
        Some(&0),
        "a nameless physical ordinal remains joinable"
    );
    assert!(slot.name.is_none(), "an absent name stays absent");
    let table = prepared.into_table();
    assert_eq!(
        serde_json::to_vec(&table)?,
        typed,
        "staging keeps the same table bytes and stable equal-key order"
    );
    assert!(
        table.stored_bytes() > 0,
        "nonempty staging accounts for its compressed bytes"
    );

    let empty = ItemPortStaging::new(directory.path(), None)
        .finish(&forge)?
        .into_table();
    let typed = CatalogTable::<ItemPortContainer<'_>>::new(&forge, &[CONTAINER], Vec::new());
    assert_eq!(
        serde_json::to_vec(&empty)?,
        serde_json::to_vec(&typed)?,
        "empty availability follows the same declarations"
    );
    assert_eq!(
        empty.stored_bytes(),
        0,
        "empty staging creates no compressed file"
    );
    Ok(())
}

#[test]
fn staged_ports_reject_out_of_order_record_drain() -> TestResult {
    let directory = tempfile::tempdir()?;
    let sources = SourceUses::new();
    let mut staged = ItemPortStaging::new(directory.path(), None);
    staged.append(row(&sources, 1, 0, true))?;
    assert!(
        matches!(
            staged.append(row(&sources, 0, 1, false)),
            Err(CatalogError::Accounting(_))
        ),
        "a reordered record drain is rejected rather than changing catalog byte order"
    );
    Ok(())
}

#[test]
fn duplicate_nameless_physical_positions_still_fail_the_alignment_index() -> TestResult {
    let directory = tempfile::tempdir()?;
    let bytes = fixture();
    let forge = DataForge::open(&bytes)?;
    let sources = SourceUses::new();
    let mut staged = ItemPortStaging::new(directory.path(), None);
    staged.append(row(&sources, 0, 1, true))?;
    staged.append(row(&sources, 0, 1, true))?;
    let mut rows = ItemAlignmentRows::default();
    rows.install_staged_ports(staged, &forge)?;
    assert!(
        matches!(AlignmentIndex::new(&rows), Err(CatalogError::Accounting(_))),
        "repeat physical slots are rejected even if they have no text name"
    );
    Ok(())
}

#[test]
fn unspooled_original_ports_have_the_same_alignment_links_as_the_producer() -> TestResult {
    let bytes = entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "FixturePortOwner", "fixture/port_owner.xml")
                .port(TestPort::new("fixture_slot"))
                .record(),
        )
        .build();
    ship_pool_test_input::with_input(&bytes, Vec::new(), |archive, _, forge, input| {
        let record = forge
            .records()
            .next()
            .ok_or("one original fixture Entity")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let sources = TextSources::read_languages(archive, &[])?;
        let texts = Texts::parse(&sources)?;
        let registry = CatalogTextRegistry::default();
        let projection = CatalogProjection::new(&texts, input, &registry);
        let options = AlignmentOptions::read(forge, None)?;
        let (ports, notes) = catalog_entity_configurations::read_item_ports(
            &tree,
            &projection,
            &admission,
            &options,
            None,
        )?;
        assert!(
            notes.is_empty(),
            "the original fixture port has no read failures"
        );
        let resolved = LoadoutResolver::new(forge).resolve(&record)?;
        let position = resolved
            .ports()
            .first()
            .and_then(|port| port.definition())
            .ok_or("resolver must retain the authored port definition")?
            .position();
        let rows = ItemAlignmentRows {
            ports,
            ..Default::default()
        };
        equal_port_link(forge, rows, position)
    })
}

fn equal_port_link(
    forge: &DataForge<'_>,
    mut rows: ItemAlignmentRows<'_>,
    position: PortDefinitionPosition,
) -> TestResult {
    let memory = AlignmentIndex::new(&rows)?.port(position)?;
    let physical = memory
        .physical_port()?
        .ok_or("unspooled original port coordinates")?;
    assert_eq!(physical.owner_record_index, position.record_index());
    assert_eq!(
        usize::try_from(physical.component_ordinal)?,
        position.component_ordinal()
    );
    assert_eq!(
        usize::try_from(physical.port_ordinal)?,
        position.port_ordinal()
    );

    let directory = tempfile::tempdir()?;
    let mut staging = ItemPortStaging::new(directory.path(), None);
    rows.stage_ports(&mut staging)?;
    rows.install_staged_ports(staging, forge)?;
    let staged = AlignmentIndex::new(&rows)?.port(position)?;
    assert_eq!(
        serde_json::to_vec(&memory)?,
        serde_json::to_vec(&staged)?,
        "both storage forms retain exactly the same original link and immediate source references",
    );
    Ok(())
}

fn fixture() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("OtherDeclaredType"))
        .build()
}

fn row(
    sources: &SourceUses,
    record: u32,
    component: u32,
    present: bool,
) -> ItemPortContainer<'static> {
    let ports = if present {
        let slot = ItemPort {
            port_ordinal: sources.value(SourceValue::Present(0), &[], true),
            name: sources.value(SourceValue::Absent, &[], true),
            display_name: sources.value(SourceValue::Absent, &[], true),
            name_use: None,
            restrictions: PortRestrictions::fixture(sources),
            attach_axes_to_align: AlignmentAxis {
                label: sources.value(SourceValue::Absent, &[], true),
                option_status: None,
            },
        };
        SourceValue::Present(vec![sources.value(SourceValue::Present(slot), &[], true)])
    } else {
        SourceValue::Absent
    };
    ItemPortContainer {
        owner: ComponentOwner::fixture(sources, record, component),
        ports: sources.value(ports, &[], true),
        ports_use: None,
    }
}
