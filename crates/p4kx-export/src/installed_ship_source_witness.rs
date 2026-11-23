// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, PhysicalSourceKind, Record, SourceInventory,
};
use p4kx_testkit::watchdog;
use serde::Serialize;

use crate::Kind;
use crate::build::installed_ship_projection::{self, Projection};
use crate::catalog_queries::equipment::ShipSourceSnapshot;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};

use super::verify::{Graph, Proof};
use super::{TestResult, binding, open_archive, receipt, wire};

#[path = "installed_ship_source_fields.rs"]
mod fields;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    schema_version: u8,
    build_sha256: String,
    archive_bytes: u64,
    selected_ship_records: usize,
    verified_cells: usize,
    sources: usize,
    uses: usize,
    producer_record_inspections: usize,
    producer_file_operations: usize,
    selected_coverage: [usize; 4],
    dependencies_complete: bool,
    metrics: [fields::Metric; 3],
    full_catalog: bool,
    media: bool,
    locale_payloads: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_ship_source_matches_scoped_catalog_projection() -> TestResult {
    watchdog::start();
    let destination = receipt::destination()?;
    let archive = open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read installed DataForge Entry")?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "bind installed Ship SDK session")?;
    let parsed = session.parsed(None).map_err(|_| "open parsed Ship view")?;
    binding(&archive, &session, &parsed)?;
    let record = select(&parsed.forge)?;
    let expected = installed_ship_projection::project(
        &archive,
        &entry,
        &parsed.forge,
        &record,
        |row, public, protected| fields::Wire::read(row, wire(public)?, wire(protected)?),
    )?;
    watch
        .check()
        .map_err(|_| "Archive changed during Ship projection")?;
    let report = parsed
        .with_ship_sources(
            CatalogRecordSelection::RecordIndex(record.index()),
            None,
            |snapshot| inspect(&archive, &entry, &record, &expected, &snapshot),
        )
        .map_err(|_| "installed Ship SDK outer completion failed")??;
    watch
        .check()
        .map_err(|_| "Archive changed before private Ship receipt publication")?;
    receipt::publish(&destination, &report)
}

fn inspect(
    archive: &Archive,
    entry: &DataForgeEntry,
    record: &Record<'_>,
    expected: &Projection<fields::Wire>,
    snapshot: &ShipSourceSnapshot<'_>,
) -> TestResult<Receipt> {
    if snapshot.build() != archive.build()
        || !snapshot.requested_languages().is_empty()
        || !snapshot.available_languages().is_empty()
        || !snapshot.localization_notes().is_empty()
    {
        return Err("Ship SDK must preserve Build binding and the empty locale policy".into());
    }
    let [row] = snapshot.rows() else {
        return Err("exactly one physical SDK Ship row is required".into());
    };
    let inventory = snapshot.source_inventory();
    if row.source().record_index().value() != Some(&record.index())
        || row.source().guid().value().copied() != Some(record.guid())
    {
        return Err("SDK row differs from the exact original Ship owner".into());
    }
    let fields = fields::Wire::read(
        row.direct_row(),
        wire(snapshot.public_names())?,
        wire(snapshot.game_texts())?,
    )?;
    fields.compare(&expected.fields)?;
    let cells = source_closure(archive, entry, inventory, &fields, record.index())?;
    source_closure(
        archive,
        entry,
        &expected.inventory,
        &expected.fields,
        record.index(),
    )?;
    Ok(Receipt {
        schema_version: 1,
        build_sha256: receipt::build_digest(archive)?,
        archive_bytes: archive.file_len(),
        selected_ship_records: snapshot.rows().len(),
        verified_cells: cells,
        sources: inventory.source_count(),
        uses: inventory.use_count(),
        producer_record_inspections: expected.record_inspections,
        producer_file_operations: expected.file_operations,
        selected_coverage: receipt::coverage(snapshot.coverage()),
        dependencies_complete: snapshot.dependency_coverage().is_complete(),
        metrics: fields::metrics(row, inventory, record.index())?,
        full_catalog: false,
        media: false,
        locale_payloads: 0,
    })
}

fn source_closure(
    archive: &Archive,
    entry: &DataForgeEntry,
    inventory: &SourceInventory,
    fields: &fields::Wire,
    index: u32,
) -> TestResult<usize> {
    let mut roots = Vec::new();
    for source in inventory.iter_sources() {
        let PhysicalSourceKind::ArchiveEntry(original) = source.kind() else {
            continue;
        };
        if !source.ancestors().is_empty() {
            return Err("this Ship witness requires root-Archive Entry evidence".into());
        }
        let index = original.entry_index();
        let admitted = archive
            .entries()
            .get(index)
            .filter(|entry| entry.index() == index)
            .or_else(|| {
                archive
                    .entries()
                    .iter()
                    .find(|entry| entry.index() == index)
            })
            .ok_or("reported physical Entry is missing from the original Archive")?;
        roots.push(
            archive
                .entry_origin(admitted)
                .ok_or("original Entry origin unavailable")?,
        );
    }
    let proof = Proof::read_with_payload(entry.origin(), &roots)?;
    proof.mapped_leaf(inventory)?;
    let graph = Graph::read(inventory)?;
    graph.physical_closure(archive, &proof)?;
    fields.verify(&graph, index)
}

fn select<'source>(forge: &'source DataForge<'source>) -> TestResult<Record<'source>> {
    let selector = std::env::var("P4KX_CATALOG_SHIP_SOURCE_RECORD")
        .map_err(|_| "set a private literal Ship Record path or class selector")?;
    if selector.is_empty() || selector.contains(['*', '?']) {
        return Err("the Ship witness needs a literal original selector".into());
    }
    let record = if let Some(class) = selector.strip_prefix("class:") {
        if class.is_empty() {
            return Err("the Ship class selector is empty".into());
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
    let kind = Kind::Ships;
    let path = record
        .record_path()
        .map_err(|_| "original Ship path unavailable")?;
    let source_type = record
        .type_name()
        .map_err(|_| "original Ship type unavailable")?;
    let name = record
        .name()
        .map_err(|_| "original Ship name unavailable")?;
    if !kind.records().selects(path, source_type)
        || crate::selection::is_kind_leftover(kind, path, name)
    {
        return Err("nominated Record is outside canonical Ship admission".into());
    }
    Ok(record)
}

fn unique<'source>(
    mut candidates: impl Iterator<Item = Record<'source>>,
) -> TestResult<Record<'source>> {
    let record = candidates.next().ok_or("selected physical Ship missing")?;
    if candidates.next().is_some() {
        return Err("the nominated selector has multiple physical owners".into());
    }
    Ok(record)
}
