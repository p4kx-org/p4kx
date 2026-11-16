// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourceUses, SourceValue};
use p4kx_testkit::{DataForgeBuilder, TestStruct};

use super::super::{AlignmentAxis, AlignmentIndex, ItemAlignmentRows, ItemPort, PortRestrictions};
use super::{CONTAINER, ComponentOwner, ItemPortContainer, ItemPortStaging};
use crate::CatalogError;
use crate::catalog_table::CatalogTable;

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
