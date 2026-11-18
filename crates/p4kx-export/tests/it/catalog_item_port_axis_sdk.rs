// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use p4kx_export::catalog_queries::{
    self, AttachmentAxisOptionStatus, CatalogLanguageSelection, CatalogRecordSelection, ItemPort,
    ItemPortSnapshot,
};
use p4kx_extraction::{Archive, DataForgeEntry, Filter, SourceInventory, SourceValueState};
use serde_json::Value;

use super::{Layout, OWNER, PORTS, TestResult, common, proof};
use crate::canonical_header::wire::Wire;

#[test]
fn item_port_axis_status_getter_matches_canonical_cells_and_original_sources() -> TestResult {
    use AttachmentAxisOptionStatus::{Declared, Undeclared};
    use SourceValueState::{Absent, Present, Unreadable};

    for (layout, statuses, labels) in [
        (
            Layout::Complete,
            [Some(Declared), Some(Undeclared)],
            [Present, Present],
        ),
        (Layout::Absent, [None, None], [Absent, Absent]),
        (
            Layout::Unreadable,
            [Some(Declared), None],
            [Present, Unreadable],
        ),
    ] {
        for (origin, marker) in [
            (common::DcbOrigin::Plain, Some(false)),
            (common::DcbOrigin::Encrypted, Some(true)),
            (common::DcbOrigin::Unknown, None),
        ] {
            let (_folder, archive) = common::open(&super::database(layout), origin)?;
            let entry = DataForgeEntry::read(&archive)?;
            let catalog = canonical(&archive)?;
            let [expected] = proof::rows(&catalog, PORTS)? else {
                return Err("one selected physical port-container row".into());
            };
            catalog_queries::with_item_ports_from(
                &archive,
                &entry,
                CatalogRecordSelection::Guid(common::guid(OWNER).parse()?),
                CatalogLanguageSelection::Selected(&[]),
                None,
                |snapshot| visit_axes(&snapshot, expected, statuses, labels, marker),
            )??;
            proof::closure(&catalog)?;
        }
    }
    Ok(())
}

fn canonical(archive: &Archive) -> TestResult<Value> {
    let filter = Filter::parse("*align_owner*")?;
    let mut options = common::options();
    options.filter = Some(&filter);
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn visit_axes(
    snapshot: &ItemPortSnapshot<'_>,
    expected: &Value,
    statuses: [Option<AttachmentAxisOptionStatus>; 2],
    labels: [SourceValueState; 2],
    marker: Option<bool>,
) -> TestResult {
    let [container] = snapshot.rows.as_slice() else {
        return Err("one selected physical port-container row".into());
    };
    let ports = container.ports().value().ok_or("readable ordered ports")?;
    let expected = proof::rows(expected, "/ports/value")?;
    assert_eq!(ports.len(), 2, "retain both original physical port slots");
    assert_eq!(ports.len(), expected.len(), "canonical slot count agrees");
    let cases = ports
        .iter()
        .zip(expected)
        .zip(statuses.into_iter().zip(labels));
    for (ordinal, ((cell, expected), (status, label))) in cases.enumerate() {
        let port = cell.value().ok_or("readable original physical port slot")?;
        let axis = proof::at(expected, "/value/attachAxesToAlign")?;
        let wire = Wire::new(axis, &snapshot.source_inventory);
        wire.field("label")?.leaf(port.alignment_axis())?;
        assert_eq!(port.alignment_axis().state(), label, "axis label {ordinal}");
        visit_membership(port, axis, &snapshot.source_inventory, status, marker)?;
    }
    Ok(())
}

fn visit_membership(
    port: &ItemPort<'_>,
    canonical: &Value,
    inventory: &SourceInventory,
    expected: Option<AttachmentAxisOptionStatus>,
    marker: Option<bool>,
) -> TestResult {
    let status = port.alignment_axis_option_status();
    assert_eq!(status.is_some(), expected.is_some(), "axis status presence");
    if let Some(status) = status {
        assert_eq!(status.value(), expected.as_ref(), "typed axis membership");
        assert_eq!(
            status.origin().contains_encrypted_source(),
            marker,
            "axis membership origin"
        );
        assert!(
            !status.origin().source_uses_refs().is_empty(),
            "axis membership uses original evidence"
        );
        Wire::new(canonical, inventory)
            .field("optionStatus")?
            .leaf(status)?;
    } else {
        assert!(
            proof::at(canonical, "/optionStatus")?.is_null(),
            "unavailable axis label has no invented membership"
        );
    }
    Ok(())
}
