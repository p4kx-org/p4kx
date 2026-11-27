// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::CatalogLanguageSelection;
use p4kx_export::catalog_queries::equipment::resource_topology::{
    ResourceTopologyDependencyRole, ResourceTopologyPlacementSnapshot, ResourceTopologyRootState,
    read_resource_topology_placements,
};
use p4kx_testkit::TestEntity;
use serde_json::{Value, json};

use super::{catalog_from, fixture, proof};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::sdk_full_proof as full_proof;

#[test]
fn scoped_topology_matches_all_three_canonical_tables_and_source_uses() -> TestResult {
    for source in [
        fixture::GraphSource::Database,
        fixture::GraphSource::Vehicle,
    ] {
        let (_directory, archive) = fixture::graph_archive_for(source)?;
        let catalog = catalog_from(&archive)?;
        let session = full_proof::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let parsed = session.parsed(None)?;
        for record in [0, 3] {
            let snapshot = parsed.read_resource_topology_placements(record, None)?;
            assert_eq!(
                snapshot.root_state,
                ResourceTopologyRootState::Resolved,
                "the query admits the exact physical Ship Record"
            );
            assert_eq!(
                snapshot
                    .loadout
                    .as_ref()
                    .map(p4kx_export::catalog_queries::equipment::Loadout::record_index),
                Some(record),
                "the snapshot retains its own root and ordered canonical Loadout"
            );
            check_placement_parity(&snapshot, &catalog, record)?;
            if record == 0 {
                check_host_dependencies(&snapshot)?;
            }
        }
    }
    Ok(())
}

fn check_placement_parity(
    snapshot: &ResourceTopologyPlacementSnapshot,
    catalog: &Value,
    record: u32,
) -> TestResult {
    let placements = serde_json::to_value(&snapshot.placements)?;
    let inventory = json!({"provenance": &snapshot.source_inventory});
    for (table, root_path) in [
        ("resourceNetworkNodes", "/position/loadoutRecordIndex"),
        ("resourceNetworkLinkBindings", "/context/loadoutRecordIndex"),
        ("roomAssociationBindings", "/context/loadoutRecordIndex"),
    ] {
        let original = proof::at(catalog, &format!("/placements/{table}"))?;
        let scoped = proof::at(&placements, &format!("/{table}"))?;
        assert_eq!(
            proof::at(scoped, "/availability")?,
            proof::at(original, "/availability")?,
            "availability comes from the same declaration source types for {table}"
        );
        let selected: Vec<_> = proof::rows(original, "/rows")?
            .iter()
            .filter(|row| row.pointer(root_path).and_then(Value::as_u64) == Some(u64::from(record)))
            .collect();
        assert_eq!(
            proof::with_origins(proof::at(scoped, "/rows")?.clone(), &inventory)?,
            proof::with_origins(serde_json::to_value(selected)?, catalog)?,
            "every {table} row retains its ordered physical identity and source uses"
        );
    }
    Ok(())
}

fn check_host_dependencies(snapshot: &ResourceTopologyPlacementSnapshot) -> TestResult {
    let reached = &snapshot.declaration_dependencies;
    let docked: Vec<_> = reached
        .iter()
        .filter(|dependency| dependency.record_index == 3)
        .map(|dependency| dependency.role)
        .collect();
    assert_eq!(
        docked,
        [ResourceTopologyDependencyRole::ResourceComponents],
        "the host reads resources on the docked target without walking its declaring context"
    );
    let relay_links = reached.iter().filter(|dependency| {
        dependency.record_index == 1
            && dependency.role == ResourceTopologyDependencyRole::LinkDeclarations
    });
    assert_eq!(
        relay_links.count(),
        1,
        "both installed relay instances reuse the same physical declaration reads"
    );
    let root = snapshot.loadout.as_ref().ok_or("selected host")?;
    let position = snapshot
        .placements
        .resource_network_nodes()
        .rows
        .iter()
        .find(|node| {
            node.vehicle_boundary()
                .and_then(p4kx_extraction::SourcedValue::value)
                == Some(&true)
        })
        .ok_or("positive authored docked attachment")?
        .position();
    assert_eq!(
        position.loadout_record_index(),
        root.record_index(),
        "the docked attachment node has the host's physical address"
    );
    Ok(())
}

#[test]
fn scoped_topology_distinguishes_unselected_roots_and_repeated_guids() -> TestResult {
    let database = fixture::physical_identity_database();
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let catalog = catalog_from(&archive)?;
    for (index, expected) in [
        (0, ResourceTopologyRootState::NotAdmitted),
        (u32::MAX, ResourceTopologyRootState::MissingRecord),
    ] {
        let snapshot = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?
            .read_resource_topology_placements(index, None)?;
        assert_eq!(
            snapshot.root_state, expected,
            "the physical root selection is explicit"
        );
        assert_eq!(
            snapshot.placements.resource_network_nodes().rows.len(),
            0,
            "an unselected physical Record emits no placements"
        );
        assert_eq!(
            snapshot.declaration_dependencies.len(),
            0,
            "an unselected root requires no declaration reads"
        );
    }
    let snapshot = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?
        .read_resource_topology_placements(2, None)?;
    assert_eq!(
        snapshot.root_state,
        ResourceTopologyRootState::Resolved,
        "selection resolves the exact same-GUID twin"
    );
    assert!(
        snapshot
            .declaration_dependencies
            .iter()
            .all(|source| source.record_index == 2),
        "the earlier Item and same-GUID twin are outside the projected source roles"
    );
    check_placement_parity(&snapshot, &catalog, 2)
}

#[test]
fn scoped_topology_admits_a_ship_with_an_empty_complete_root() -> TestResult {
    let database = fixture::entity_schema().record(
        TestEntity::new(
            0x1166_0050,
            "FixtureEmptyTopology",
            "libs/foundry/records/entities/spaceships/fixture_empty_topology.xml",
        )
        .vehicle("", "")
        .record(),
    );
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let snapshot = read_resource_topology_placements(&archive, 0, None)?;
    assert_eq!(
        snapshot.root_state,
        ResourceTopologyRootState::Resolved,
        "an admitted Ship with a complete empty graph still has a physical root"
    );
    assert_eq!(
        snapshot
            .loadout
            .as_ref()
            .ok_or("selected empty Ship")?
            .ports()
            .len(),
        0,
        "the canonical root retains its empty resolved port inventory"
    );
    assert_eq!(
        snapshot.placements.resource_network_nodes().rows.len(),
        0,
        "an empty root has no manufactured node"
    );
    assert!(
        snapshot
            .declaration_dependencies
            .iter()
            .all(|source| source.state
                == p4kx_export::catalog_queries::equipment::AlignmentDependencyState::Complete),
        "the absence of links, rooms and resources was read completely"
    );
    Ok(())
}
