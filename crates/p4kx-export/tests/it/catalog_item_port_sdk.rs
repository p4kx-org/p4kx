// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, DatasetAvailability, ItemPortSnapshot,
    with_item_ports,
};
use p4kx_export::{CatalogError, build_catalog};
use p4kx_extraction::{CancelToken, DataForgeEntry, Filter};
use serde_json::Value;

use super::{Layout, OWNER, PORTS, TestResult, common, database, proof};

#[test]
fn selected_item_ports_keep_canonical_rows_and_direct_encrypted_origins() -> TestResult {
    let (_folder, archive) =
        common::open(&database(Layout::Complete), common::DcbOrigin::Encrypted)?;
    let entry = DataForgeEntry::read(&archive)?;
    let filter = Filter::parse("*align_owner*")?;
    let mut options = common::options();
    options.filter = Some(&filter);
    let built = build_catalog(&archive, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let expected = proof::rows(&catalog, PORTS)?;
    assert_eq!(expected.len(), 1, "exactly the selected owner's container");
    full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_item_ports(
        CatalogRecordSelection::Guid(common::guid(OWNER).parse()?),
        None,
        |snapshot| {
            assert!(matches!(
                snapshot.availability,
                DatasetAvailability::Available
            ));
            let [row] = snapshot.rows.as_slice() else {
                return Err("one selected physical port container".into());
            };
            assert_eq!(row.owner_record_index().value(), Some(&0));
            assert_eq!(row.component_ordinal().value(), Some(&1));
            assert_eq!(&[serde_json::to_value(row)?], expected);
            check_names_and_sources(&snapshot)
        },
    )?
}

fn check_names_and_sources(snapshot: &ItemPortSnapshot<'_>) -> TestResult {
    let [container] = snapshot.rows.as_slice() else {
        return Err("one physical port container".into());
    };
    let ports = container.ports().value().ok_or("authored port list")?;
    assert_eq!(ports.len(), 2, "both authored port slots are present");
    for (ordinal, (cell, name)) in ports.iter().zip(["mount", "future"]).enumerate() {
        let port = cell.value().ok_or("authored port slot")?;
        assert_eq!(
            port.port_ordinal().value(),
            Some(&u32::try_from(ordinal)?),
            "the physical port position is stable"
        );
        assert_eq!(
            port.name().value(),
            Some(&name),
            "the authored port name is retained"
        );
        let origin = port.name().origin();
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(true),
            "the port name uses the encrypted DCB"
        );
        assert!(!origin.source_uses_refs().is_empty(), "actual source uses");
        for id in origin.source_uses_refs() {
            assert!(
                snapshot
                    .source_inventory
                    .iter_uses()
                    .any(|row| row.id() == id),
                "exact field origins close within this result"
            );
        }
    }
    Ok(())
}

#[test]
fn item_port_scope_retains_its_faults_and_excludes_unrelated_component_failures() -> TestResult {
    for (layout, expected) in [(Layout::UnrelatedInvalid, 0), (Layout::Unreadable, 1)] {
        let (_folder, archive) = common::open(&database(layout), common::DcbOrigin::Plain)?;
        assert!(
            matches!(
                build_catalog(&archive, common::options().with_full_provenance()),
                Err(CatalogError::IncompleteExport(_))
            ),
            "the full export observes the invalid field"
        );
        let (count, incomplete, unreadable) = with_item_ports(
            &archive,
            CatalogRecordSelection::RecordIndex(0),
            CatalogLanguageSelection::Selected(&[]),
            None,
            |result| {
                (
                    result.rows.len(),
                    result.coverage.incomplete_fields,
                    result.coverage.unreadable_bodies,
                )
            },
        )?;
        assert_eq!(count, 1, "retain the actual port component");
        assert_eq!(
            incomplete.len(),
            expected,
            "only note failures in the requested projector"
        );
        assert!(
            unreadable.is_empty(),
            "the Record tree was successfully read"
        );
    }
    Ok(())
}

#[test]
fn cancelled_item_ports_never_invoke_the_callback() -> TestResult {
    let (_folder, archive) = common::open(&database(Layout::Complete), common::DcbOrigin::Plain)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let result: Result<(), CatalogError> = with_item_ports(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        CatalogLanguageSelection::Selected(&[]),
        Some(&cancel),
        |_| panic!("a cancelled read has no result"),
    );
    assert!(matches!(result, Err(CatalogError::Cancelled)));
    Ok(())
}
