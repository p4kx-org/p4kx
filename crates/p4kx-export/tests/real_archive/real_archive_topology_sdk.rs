// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ArchiveFiles;
use p4kx_export::catalog_queries::equipment::AlignmentDependencyState;
use p4kx_export::catalog_queries::equipment::resource_topology::{
    ResourceTopologyPlacementSnapshot, ResourceTopologyRootState,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries,
};
use p4kx_export::catalog_schema;
use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use super::{Origins, Ship, TestResult, bindings, build, proof, select};
use crate::real_archive_catalog_table_parity;
use crate::real_archive_verification::print_line;

#[path = "real_archive_topology_sdk_memory.rs"]
mod memory;
#[path = "real_archive_topology_sdk_receipt.rs"]
mod receipt;
#[path = "real_archive_topology_sdk_routes.rs"]
mod routes;

#[test]
#[ignore = "real Archive: configure P4KX_DATA_P4K and a new private topology receipt"]
fn installed_topology_sdk_matches_source_selected_roots() -> TestResult {
    let destination = receipt::destination()?;
    if destination
        .try_exists()
        .map_err(|_| "inspect the private topology receipt destination")?
    {
        return Err("topology source verification needs a new private receipt".into());
    }
    let archive = open_archive()?;
    let stamp = receipt::Stamp::read(&archive)?;
    stamp.check_requested_change()?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge Entry")?;
    let original = entry.open().map_err(|_| "parse original DataForge")?;
    let files = ArchiveFiles::new(&archive);
    let (fixed, multi) = select(&original, &files, &watch)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "bind the topology query to the original Entry")?;
    let parsed = session
        .parsed(None)
        .map_err(|_| "parse the SDK DataForge view")?;
    let validator = placement_validator()?;
    let samples = [
        verify_root(&archive, &entry, &original, &parsed, &fixed, &validator)?,
        verify_root(&archive, &entry, &original, &parsed, &multi, &validator)?,
    ];
    watch
        .check()
        .map_err(|_| "Archive changed during topology verification")?;
    receipt::Report::new(stamp, samples).publish(&destination)?;
    print_line(
        "Scoped topology SDK: two independently selected roots passed direct producer, original dependency and Loadout issue checks",
    );
    Ok(())
}

fn open_archive() -> TestResult<Archive> {
    let path = real_archive_path().map_err(|_| "configure one installed Archive")?;
    let key = real_archive_key().map_err(|_| "configure the installed Archive key")?;
    Archive::open_with_key(path, Some(key)).map_err(|_| "open the installed Archive".into())
}

fn verify_root(
    archive: &Archive,
    entry: &DataForgeEntry,
    original: &DataForge<'_>,
    parsed: &ParsedCatalogQueries<'_>,
    ship: &Ship<'_>,
    validator: &jsonschema::Validator,
) -> TestResult<receipt::Sample> {
    let root = ship.tree.record_index();
    let snapshot = parsed
        .read_resource_topology_placements(root, None)
        .map_err(|_| "read the selected physical topology root")?;
    check_selection(archive, root, &snapshot)?;
    routes::check(ship, &snapshot, original, entry.origin())?;
    let scoped = serde_json::to_value(&snapshot.placements)?;
    if !validator.is_valid(&scoped) {
        return Err("scoped topology tables fail the canonical wire schema".into());
    }
    let catalog = build(archive, entry, ship.tree.record_path())?;
    let origins = Origins::read(&catalog, entry.origin())?;
    let direct = bindings::compare(&catalog, &origins, &ship.tree, &ship.assembly, &ship.relays)?;
    direct_parity(&snapshot, &scoped, &catalog, root)?;
    check_scope_closure(&snapshot, &scoped)?;
    Ok(receipt::Sample::new(
        root,
        &snapshot,
        &direct,
        ship.relays.len(),
    ))
}

fn check_selection(
    archive: &Archive,
    root: u32,
    snapshot: &ResourceTopologyPlacementSnapshot,
) -> TestResult {
    if snapshot.build.as_ref() != archive.build() {
        return Err("topology result Build differs from the installed Archive".into());
    }
    match snapshot.root_state {
        ResourceTopologyRootState::Resolved => {}
        ResourceTopologyRootState::MissingRecord => {
            return Err("selected physical root is missing".into());
        }
        ResourceTopologyRootState::NotAdmitted => {
            return Err("selected physical root is outside Ship admission".into());
        }
        ResourceTopologyRootState::UnreadableHeader => {
            return Err("selected physical root header is unreadable".into());
        }
        _ => return Err("selected physical root could not be resolved".into()),
    }
    let loadout = snapshot
        .loadout
        .as_ref()
        .ok_or("resolved root lacks its Loadout")?;
    if loadout.record_index() != root || loadout.source_record_index().value() != Some(&root) {
        return Err("the returned Loadout differs from the requested physical Record".into());
    }
    let expected_incomplete: &[u32] = if loadout.issues().is_empty() {
        &[]
    } else {
        std::slice::from_ref(&root)
    };
    if !snapshot.coverage.unreadable_headers.is_empty()
        || !snapshot.coverage.unreadable_bodies.is_empty()
        || snapshot.coverage.incomplete_fields != expected_incomplete
        || !snapshot.loadout_dependencies.is_complete()
    {
        let resolver = &snapshot.loadout_dependencies.resolver;
        let unavailable_alignment = snapshot
            .loadout_dependencies
            .alignment
            .iter()
            .filter(|dependency| {
                !matches!(
                    dependency.state,
                    AlignmentDependencyState::Complete | AlignmentDependencyState::NotAdmitted
                )
            })
            .count();
        return Err(format!(
            "selected topology inputs incomplete: unreadable headers {}, unreadable bodies {}, incomplete owner projections {}, resolver directory {}, failed Records {}, failed Entries {:?}, incomplete resolver records {}, incomplete resolver fields {}, unavailable alignment {}",
            snapshot.coverage.unreadable_headers.len(),
            snapshot.coverage.unreadable_bodies.len(),
            snapshot.coverage.incomplete_fields.len(),
            resolver.directory_complete(),
            resolver.failed_records().len(),
            resolver.failed_entries().iter().map(|entry| (entry.operation(), entry.stage())).collect::<Vec<_>>(),
            resolver.incomplete_records().len(),
            resolver.incomplete_fields(),
            unavailable_alignment,
        )
        .into());
    }
    routes::check_declaration_reads(snapshot, root)
}

const TABLES: [(&str, &str); 3] = [
    ("resourceNetworkNodes", "CatalogTableOfResourceNetworkNode"),
    (
        "resourceNetworkLinkBindings",
        "CatalogTableOfResourceNetworkLinkBinding",
    ),
    (
        "roomAssociationBindings",
        "CatalogTableOfRoomAssociationBinding",
    ),
];

fn placement_validator() -> TestResult<jsonschema::Validator> {
    let mut schema = serde_json::to_value(catalog_schema())?;
    let definitions = schema
        .as_object_mut()
        .and_then(|schema| schema.remove("$defs"))
        .ok_or("canonical catalog schema definitions")?;
    let properties: serde_json::Map<_, _> = TABLES
        .into_iter()
        .map(|(table, name)| (table.to_owned(), json!({"$ref": format!("#/$defs/{name}")})))
        .collect();
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": TABLES.map(|(table, _)| table),
        "properties": properties,
        "$defs": definitions,
    });
    jsonschema::validator_for(&schema)
        .map_err(|_| "canonical scoped placement schema could not be compiled".into())
}

fn direct_parity(
    snapshot: &ResourceTopologyPlacementSnapshot,
    scoped: &Value,
    catalog: &Value,
    root: u32,
) -> TestResult {
    let mut origins = real_archive_catalog_table_parity::Origins::default();
    let sdk = snapshot
        .loadout
        .as_ref()
        .ok_or("resolved Loadout for direct proof")?;
    let indexed = proof::rows(catalog, "/loadouts")?
        .iter()
        .find(|row| {
            row.pointer("/sourceRecordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(root))
        })
        .ok_or("physical Loadout in the direct producer scope")?;
    let root_cell = real_archive_catalog_table_parity::wire_row(sdk.source_record_index())?;
    if &root_cell != at(indexed, "/sourceRecordIndex")? {
        return Err("SDK physical root header or its immediate source differs".into());
    }
    origins.extend(&snapshot.source_inventory, &[root_cell])?;
    for (table, _) in TABLES {
        let actual = at(scoped, &format!("/{table}"))?;
        let expected = at(catalog, &format!("/placements/{table}"))?;
        if at(actual, "/availability")? != at(expected, "/availability")? {
            return Err(format!("declaration availability differs for {table}").into());
        }
        if table != "resourceNetworkNodes" {
            check_direct_bindings(actual, expected, root, table, snapshot, &mut origins)?;
        }
    }
    origins.verify_in_document(catalog)
}

fn check_direct_bindings(
    actual: &Value,
    expected: &Value,
    root: u32,
    table: &str,
    snapshot: &ResourceTopologyPlacementSnapshot,
    origins: &mut real_archive_catalog_table_parity::Origins,
) -> TestResult {
    let actual = direct_bindings(actual, root)?;
    let expected = direct_bindings(expected, root)?;
    if actual != expected {
        return Err(
            format!("direct canonical bindings or contributor IDs differ for {table}").into(),
        );
    }
    if !actual.is_empty() {
        origins.extend(
            &snapshot.source_inventory,
            &actual.into_iter().cloned().collect::<Vec<_>>(),
        )?;
    }
    Ok(())
}

fn direct_bindings(value: &Value, root: u32) -> TestResult<Vec<&Value>> {
    Ok(proof::rows(value, "/rows")?
        .iter()
        .filter(|row| {
            row.pointer("/context/loadoutRecordIndex")
                .and_then(Value::as_u64)
                == Some(u64::from(root))
                && row
                    .pointer("/context/portOrdinals")
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty)
        })
        .collect())
}

fn check_scope_closure(snapshot: &ResourceTopologyPlacementSnapshot, scoped: &Value) -> TestResult {
    let mut origins = real_archive_catalog_table_parity::Origins::default();
    for (table, _) in TABLES {
        let rows = proof::rows(scoped, &format!("/{table}/rows"))?;
        if !rows.is_empty() {
            origins.extend(&snapshot.source_inventory, rows)?;
        }
    }
    Ok(())
}

fn at<'value>(value: &'value Value, path: &str) -> TestResult<&'value Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("required scope field is absent at {path}").into())
}
