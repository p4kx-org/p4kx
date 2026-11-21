// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeOrigin, PhysicalSource, Record,
    SourceEncryptionState, SourceInventory, SourceLocation, SourceUseRole, SourcedValue,
};
use p4kx_testkit::watchdog;
use serde::Serialize;

use crate::Kind;
use crate::catalog_queries::equipment::{AlignmentRole, LoadoutOwnerKind, LoadoutSnapshot};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use crate::catalog_read::original::OriginalValuePart;
use crate::contract_loadout::Loadout;
use crate::entity_selection::{self, Selected};
use crate::guid_text::CatalogGuid;
use crate::record_header_field::RecordHeaderField;

use super::verify::{Graph, Proof};
use super::{TestResult, binding, open_archive, receipt, wire};

#[path = "installed_loadout_original_port.rs"]
mod original_port;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    schema_version: u8,
    build_sha256: String,
    archive_bytes: u64,
    selected_loadout_records: usize,
    checked_dataforge_names: usize,
    checked_owner_headers: usize,
    root_ports: usize,
    reached_alignment_roles: usize,
    sources: usize,
    uses: usize,
    selected_coverage: [usize; 4],
    dependencies_complete: bool,
    name_contains_encrypted_source: Option<bool>,
    full_catalog: bool,
    locale_payloads: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_loadout_sdk_matches_one_original_port_name() -> TestResult {
    watchdog::start();
    let destination = receipt::destination()?;
    let archive = open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)
            .map_err(|_| "open installed Loadout SDK session")?;
    let parsed = session
        .parsed(None)
        .map_err(|_| "open parsed Loadout view")?;
    binding(&archive, &session, &parsed)?;
    let (record, kind) = select(&parsed.forge)?;
    let snapshot = parsed
        .read_loadouts(CatalogRecordSelection::RecordIndex(record.index()), None)
        .map_err(|_| "installed Loadout SDK outer completion failed")?;
    let report = inspect(
        &archive,
        session.dataforge_origin(),
        &record,
        kind,
        &snapshot,
    )?;
    watch
        .check()
        .map_err(|_| "Archive changed before private Loadout receipt publication")?;
    receipt::publish(&destination, &report)
}

fn inspect(
    archive: &Archive,
    origin: &DataForgeOrigin,
    record: &Record<'_>,
    kind: LoadoutOwnerKind,
    snapshot: &LoadoutSnapshot,
) -> TestResult<Receipt> {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one physical SDK Loadout owner is required".into());
    };
    if snapshot.build.as_ref() != archive.build()
        || row.entity_kind() != kind
        || row.entity_guid() != record.guid()
        || row.record_index() != record.index()
        || row.source_record_index().value() != Some(&record.index())
    {
        return Err("Loadout SDK owner differs from the original nominated Record".into());
    }
    let selected = original_port::read(record, row)?;
    if !snapshot.dependencies.alignment.iter().any(|dependency| {
        dependency.record_index == record.index() && dependency.role == AlignmentRole::Port
    }) {
        return Err("the confirmed original Port role is missing from SDK scope".into());
    }
    let inventory = &snapshot.source_inventory;
    let proof = Proof::read(origin)?;
    let leaf = proof.mapped_leaf(inventory)?;
    let source = CellProof {
        graph: Graph::read(inventory)?,
        inventory,
        leaf,
    };
    source.owner(row, record)?;
    source.port_name(record.index(), &selected)?;
    Ok(Receipt {
        schema_version: 1,
        build_sha256: receipt::build_digest(archive)?,
        archive_bytes: archive.file_len(),
        selected_loadout_records: snapshot.rows.len(),
        checked_dataforge_names: 1,
        checked_owner_headers: 3,
        root_ports: row.ports().len(),
        reached_alignment_roles: snapshot.dependencies.alignment.len(),
        sources: inventory.source_count(),
        uses: inventory.use_count(),
        selected_coverage: receipt::coverage(&snapshot.coverage),
        dependencies_complete: snapshot.dependencies.is_complete(),
        name_contains_encrypted_source: selected.name.origin().contains_encrypted_source(),
        full_catalog: false,
        locale_payloads: 0,
    })
}

struct CellProof<'inventory> {
    graph: Graph<'inventory>,
    inventory: &'inventory SourceInventory,
    leaf: &'inventory PhysicalSource,
}

struct Contribution {
    role: SourceUseRole,
    location: SourceLocation,
}

impl CellProof<'_> {
    fn owner(&self, row: &Loadout, record: &Record<'_>) -> TestResult {
        let index = record.index();
        let selected = SourceUseRole::Selector;
        let header = |field: RecordHeaderField, role| Contribution {
            role,
            location: field.location(index),
        };
        let guid = row.source_entity_guid().map(CatalogGuid);
        let kind = row.source_entity_kind();
        if guid.value() != Some(&CatalogGuid(record.guid()))
            || kind.value() != Some(&row.entity_kind())
        {
            return Err("the sourced owner fields differ from the selected original Record".into());
        }
        self.check(
            row.source_record_index(),
            &[header(RecordHeaderField::RecordIndex, selected)],
        )?;
        self.check(
            &guid,
            &[header(RecordHeaderField::Guid, SourceUseRole::Payload)],
        )?;
        self.check(
            &kind,
            &[
                header(RecordHeaderField::SourceType, selected),
                header(RecordHeaderField::RecordName, selected),
                header(RecordHeaderField::FileName, selected),
                header(RecordHeaderField::FileName, SourceUseRole::Conversion),
            ],
        )
    }

    fn port_name(&self, index: u32, port: &original_port::OriginalPort<'_>) -> TestResult {
        self.check_cell(port.name)?;
        let refs = port.name.origin().source_uses_refs();
        let mut payloads = 0;
        let mut selectors = 0;
        for used in self
            .inventory
            .iter_uses()
            .filter(|used| refs.contains(used.id()))
        {
            if used.source() != self.leaf.id() {
                return Err("the original port name cites a different physical DCB".into());
            }
            match (used.role(), used.location()) {
                (SourceUseRole::Payload, location) if location == &port.location => payloads += 1,
                (
                    SourceUseRole::Selector,
                    SourceLocation::Subresource {
                        namespace, indices, ..
                    },
                ) if matches!(
                    namespace.as_str(),
                    OriginalValuePart::NAMESPACE | RecordHeaderField::NAMESPACE
                ) && indices.first() == Some(&u64::from(index)) =>
                {
                    selectors += 1;
                }
                _ => {
                    return Err(
                        "the name lacks its original payload or declaration ancestry".into(),
                    );
                }
            }
        }
        if payloads != 1 || selectors == 0 {
            return Err(
                "one actual port Name payload and its declaration selectors are required".into(),
            );
        }
        Ok(())
    }

    fn check_cell<T: Serialize>(&self, cell: &SourcedValue<T>) -> TestResult {
        let expected_marker = match self.leaf.stored_encryption() {
            SourceEncryptionState::Encrypted => Some(true),
            SourceEncryptionState::Unencrypted => Some(false),
            _ => None,
        };
        if cell.origin().contains_encrypted_source() != expected_marker {
            return Err("Loadout field marker differs from its original DCB domain".into());
        }
        if self.graph.walk(&wire(cell)?)? != 1 {
            return Err("one closed immediate Loadout contribution set is required".into());
        }
        Ok(())
    }

    fn check<T: Serialize>(&self, cell: &SourcedValue<T>, expected: &[Contribution]) -> TestResult {
        self.check_cell(cell)?;
        let refs = cell.origin().source_uses_refs();
        if refs.len() != expected.len() {
            return Err("Loadout owner coordinates must contain exactly the specified uses".into());
        }
        for contribution in expected {
            let actual = self
                .inventory
                .iter_uses()
                .filter(|used| {
                    refs.contains(used.id())
                        && used.source() == self.leaf.id()
                        && used.role() == contribution.role
                        && *used.location() == contribution.location
                })
                .count();
            if actual != 1 {
                return Err(
                    "Loadout field lacks its exact original coordinate and use role".into(),
                );
            }
        }
        Ok(())
    }
}

fn select<'source>(
    forge: &'source DataForge<'source>,
) -> TestResult<(Record<'source>, LoadoutOwnerKind)> {
    let selector = std::env::var("P4KX_CATALOG_LOADOUT_SOURCE_RECORD")
        .map_err(|_| "set one private literal Loadout owner path or class selector")?;
    if selector.is_empty() || selector.contains(['*', '?']) {
        return Err("the Loadout witness requires a literal original owner selector".into());
    }
    let record = if let Some(class) = selector.strip_prefix("class:") {
        if class.is_empty() {
            return Err("the private Loadout class selector is empty".into());
        }
        unique(forge.records().filter(|record| {
            record.name().is_ok_and(|name| {
                crate::record_fields::unqualified_class_name(name, p4kx_assembly::ENTITY_TYPE)
                    .eq_ignore_ascii_case(class)
            })
        }))?
    } else {
        unique(forge.records_at_path(&selector))?
    };
    if record
        .type_name()
        .map_err(|_| "original Loadout owner type unavailable")?
        != p4kx_assembly::ENTITY_TYPE
    {
        return Err("nominated original Loadout owner is not an entity Record".into());
    }
    let kind = match entity_selection::select(&record, None)
        .map_err(|_| "original Loadout owner classification unavailable")?
    {
        Some(Selected::Entity(Kind::Ships)) => LoadoutOwnerKind::Ship,
        Some(Selected::Entity(Kind::Items)) => LoadoutOwnerKind::Item,
        _ => return Err("nominated original owner is outside Loadout admission".into()),
    };
    Ok((record, kind))
}

fn unique<'source>(
    mut candidates: impl Iterator<Item = Record<'source>>,
) -> TestResult<Record<'source>> {
    let owner = candidates
        .next()
        .ok_or("nominated original Loadout owner missing")?;
    if candidates.next().is_some() {
        return Err("nominated Loadout selector has multiple physical owners".into());
    }
    Ok(owner)
}
