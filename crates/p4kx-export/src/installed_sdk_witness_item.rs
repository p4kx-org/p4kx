// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::{
    Archive, ArchiveWatch, CanonicalReadObserver, DataForge, DataForgeError, InputScope, Language,
    LocalizationEntry, ProvenanceMode, Record, RecordTree, SourceLocation, Step,
};
use p4kx_testkit::watchdog;
use serde::Serialize;

use crate::catalog_queries::equipment::{
    ItemSource, ItemSourceSnapshot, observe_item_source_query,
};
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
};
use crate::catalog_read::original::OriginalValuePart;

use super::pool::check_header;
use super::verify::{self, Graph, Proof};
use super::{TestResult, binding, open_archive, receipt, wire};

#[path = "installed_sdk_item_original.rs"]
mod original_item_values;

use original_item_values::OriginalItem;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Receipt {
    schema_version: u8,
    build_sha256: String,
    archive_bytes: u64,
    body_reads: usize,
    rows: usize,
    observed_locale_payloads: usize,
    sources: usize,
    uses: usize,
    verified_cells: usize,
}

struct ReadCount {
    expected: u32,
    count: AtomicUsize,
}

impl CanonicalReadObserver for ReadCount {
    fn observe(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        if record.index() != self.expected || outcome.is_err() {
            return ControlFlow::Break(());
        }
        self.count.fetch_add(1, Ordering::Relaxed);
        ControlFlow::Continue(())
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K, P4KX_CATALOG_ITEM_TEXT_RECORD and P4KX_SDK_WITNESS_FILE"]
fn installed_item_source_sdk_matches_one_original_record() -> TestResult {
    watchdog::start();
    let destination = receipt::destination()?;
    let archive = open_archive()?;
    let watch = ArchiveWatch::new(&archive);
    let session = CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)
        .map_err(|_| "installed Item SDK session open failed")?
        .with_provenance_mode(ProvenanceMode::Full);
    let parsed = session
        .parsed(None)
        .map_err(|_| "installed Item SDK parsed view failed")?;
    binding(&archive, &session, &parsed)?;

    let original = original_item(&parsed.forge)?
        .tree()
        .map_err(|_| "selected original Item body is unreadable")?;
    let english = LocalizationEntry::read(&archive, Language::English)
        .map_err(|_| "original English locale is unreadable")?;
    let locale = english
        .origin()
        .ok_or("original English Entry binding missing")?;
    let proof = Proof::read_with_payload(session.dataforge_origin(), Some(locale))?;
    let read_count = Arc::new(ReadCount {
        expected: original.record_index(),
        count: AtomicUsize::new(0),
    });
    let stats = observe_item_source_query(
        &parsed,
        CatalogRecordSelection::RecordIndex(original.record_index()),
        read_count.clone(),
        |snapshot| check_snapshot(&archive, &original, &english, &proof, &snapshot),
    )
    .map_err(|_| "observed Item SDK query failed")??;
    let body_reads = read_count.count.load(Ordering::Relaxed);
    if body_reads != 1 {
        return Err("Item SDK query must decode exactly one physical Record".into());
    }
    watch.check().map_err(|_| "installed Archive changed")?;
    receipt::publish(
        &destination,
        &Receipt {
            schema_version: 1,
            build_sha256: receipt::build_digest(&archive)?,
            archive_bytes: archive.file_len(),
            body_reads,
            rows: 1,
            observed_locale_payloads: stats.locale_entries,
            sources: stats.sources,
            uses: stats.uses,
            verified_cells: stats.cells,
        },
    )
}

fn original_item<'source>(forge: &'source DataForge<'source>) -> TestResult<Record<'source>> {
    let path = std::env::var("P4KX_CATALOG_ITEM_TEXT_RECORD")
        .map_err(|_| "configure the exact private Item Record path")?;
    let mut records = forge.records().filter(|record| {
        record
            .record_path()
            .is_ok_and(|candidate| candidate == path)
            && record
                .type_name()
                .is_ok_and(|kind| kind == "EntityClassDefinition")
    });
    let record = records.next().ok_or("selected Item Record missing")?;
    if records.next().is_some() {
        return Err("Item Record path must identify a single physical owner".into());
    }
    Ok(record)
}

struct Stats {
    locale_entries: usize,
    sources: usize,
    uses: usize,
    cells: usize,
}

fn check_snapshot(
    archive: &Archive,
    original: &RecordTree<'_>,
    english: &LocalizationEntry,
    proof: &Proof,
    snapshot: &ItemSourceSnapshot<'_>,
) -> TestResult<Stats> {
    if snapshot.build() != archive.build()
        || !matches!(snapshot.availability(), DatasetAvailability::Available)
        || !snapshot.requested_languages().is_empty()
        || !snapshot.available_languages().is_empty()
        || !snapshot.public_names().is_empty()
        || !snapshot.game_texts().is_empty()
    {
        return Err(
            "Item source query returned unexpected Build, dataset or presentation scope".into(),
        );
    }
    verify::coverage(snapshot.coverage(), 0)?;
    let [row] = snapshot.rows() else {
        return Err("Item source query must return its one physical owner".into());
    };
    let inventory = snapshot.source_inventory();
    let leaf = proof.mapped_leaf(inventory)?;
    let graph = Graph::read(inventory)?;
    graph.physical_closure(archive, proof)?;
    let row_wire = wire(row)?;
    check_header(original, row.source(), &row_wire, &graph)?;
    let source = OriginalItem::read(original)?;
    source.check_values(row, english)?;
    source.check_coordinates(original, &row_wire, &graph)?;

    let mut locale_entries = inventory.iter_sources().filter(|source| {
        matches!(source.kind(), p4kx_extraction::PhysicalSourceKind::ArchiveEntry(entry)
                if entry.path().value().is_some_and(|path| path.eq_ignore_ascii_case(english.path())))
    });
    let locale = locale_entries
        .next()
        .ok_or("original English locale missing")?;
    if locale_entries.next().is_some() {
        return Err("original English locale must have one physical source".into());
    }
    check_lookup(
        original,
        source.attachment_index,
        row,
        snapshot,
        leaf.id(),
        locale.id(),
    )?;
    Ok(Stats {
        locale_entries: 1,
        sources: inventory.source_count(),
        uses: inventory.use_count(),
        cells: graph.walk(&row_wire)?,
    })
}

fn location(tree: &RecordTree<'_>, steps: &[Step<'_>]) -> TestResult<SourceLocation> {
    let mut ordinal = None;
    tree.root().visit_owned_values(|visit| {
        if visit.steps() == steps {
            ordinal = Some(visit.ordinal());
        }
    });
    Ok(OriginalValuePart::Value.location(
        tree.record_index(),
        u64::try_from(ordinal.ok_or("original Item source coordinate missing")?)?,
    ))
}

fn check_lookup(
    tree: &RecordTree<'_>,
    attachment: u32,
    row: &ItemSource<'_>,
    snapshot: &ItemSourceSnapshot<'_>,
    dataforge: &p4kx_extraction::PhysicalSourceRef,
    english: &p4kx_extraction::PhysicalSourceRef,
) -> TestResult {
    let expected = location(
        tree,
        &[
            Step::Field("Components"),
            Step::Element(attachment),
            Step::Field("AttachDef"),
            Step::Field("Localization"),
            Step::Field("Description"),
        ],
    )?;
    let inventory = snapshot.source_inventory();
    let references = row.item_class().origin().source_uses_refs();
    for usage in inventory
        .iter_uses()
        .filter(|usage| references.contains(usage.id()))
    {
        let SourceLocation::LocalizationLookup {
            language: Language::English,
            selector,
        } = usage.location()
        else {
            continue;
        };
        if usage.source() == english
            && usage.scope() == InputScope::Payload
            && inventory.iter_uses().any(|candidate| {
                candidate.id() == selector
                    && candidate.location() == &expected
                    && candidate.source() == dataforge
                    && candidate.scope() == InputScope::Payload
            })
        {
            return Ok(());
        }
    }
    Err("Item class lacks its original description selector and English lookup".into())
}
