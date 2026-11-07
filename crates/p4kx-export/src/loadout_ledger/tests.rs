// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{Entities, LoadoutResolver};
use p4kx_extraction::{DataForge, Record, RecordTree};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry, TestRecord, TestStruct, TestType,
    TestValue,
};

use crate::Kind;
use crate::contract_loadout::EntityKind;
use crate::export_diagnostic::{ExportDiagnosticCode, ExportDiagnosticSeverity, ExportSourceRole};
use crate::export_ledger::{ExportLedger, SourceDisposition, SourceObservation};
use crate::guid_text::CatalogGuid;
use crate::loadout::{self, Wanted};
use crate::loadout_port::PortReader;

use super::Capture;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ROOT: u128 = 0x6c00_0000_0000_4000_8000_0000_0000_0001;

fn source(record: &Record<'_>, tree: &RecordTree<'_>) -> Result<ExportLedger, crate::CatalogError> {
    let mut ledger = crate::export_field_walk::inspect(
        tree,
        record,
        Some(Kind::Items),
        ExportSourceRole::Selected,
        None,
    )?;
    ledger.observe(SourceObservation::new(
        Kind::Items,
        record,
        ExportSourceRole::Selected,
        SourceDisposition::Projected,
    ));
    Ok(ledger)
}

#[test]
fn an_explicit_zero_row_item_loadout_remains_a_successful_actual_input() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("EntityClassDefinition"))
        .record(TestRecord::new(
            ROOT,
            "EntityClassDefinition",
            "EntityClassDefinition.Synthetic",
            "synthetic/root.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let tree = record.tree()?;
    let original = source(&record, &tree)?;
    let capture = Arc::new(Capture::new(&original, None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone());
    let row = loadout::read_with(
        &mut resolver,
        &mut PortReader::new(),
        &forge,
        Wanted {
            record_index: record.index(),
            guid: CatalogGuid(record.guid()),
            kind: EntityKind::Item,
        },
        &capture,
    )
    .map_err(|failure| failure.reason)?;
    let ledger = capture.finish()?;
    assert!(
        row.is_none(),
        "the known zero-port Item branch emits no Loadout row"
    );
    let input = ledger
        .sources
        .iter()
        .find(|source| source.role == ExportSourceRole::DerivedInput)
        .ok_or("actual input missing")?;
    assert_eq!(
        input.disposition,
        SourceDisposition::Projected,
        "an explicit successful zero-row projection is not an omission"
    );
    assert!(
        ledger.derived_failures.is_empty(),
        "no secondary operation failed"
    );
    assert_eq!(
        original
            .sources
            .first()
            .ok_or("original source missing")?
            .disposition,
        SourceDisposition::Projected,
        "loadout accounting leaves the original Item source intact"
    );
    Ok(())
}

fn verify_original_projection(original: &ExportLedger) -> TestResult {
    assert_eq!(
        original
            .sources
            .first()
            .ok_or("original source missing")?
            .disposition,
        SourceDisposition::Projected,
        "the original source was not relabeled omitted"
    );
    Ok(())
}

fn wrong_type_database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Other"))
        .record(TestRecord::new(
            ROOT,
            "Other",
            "Other.Synthetic",
            "synthetic/root.xml",
        ))
        .build()
}

#[test]
fn a_failed_secondary_projection_does_not_omit_the_original_item() -> TestResult {
    let bytes = wrong_type_database();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let tree = record.tree()?;
    let original = source(&record, &tree)?;
    let capture = Arc::new(Capture::new(&original, None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone());
    assert!(
        loadout::read_with(
            &mut resolver,
            &mut PortReader::new(),
            &forge,
            Wanted {
                record_index: record.index(),
                guid: CatalogGuid(record.guid()),
                kind: EntityKind::Item
            },
            &capture
        )
        .is_err(),
        "the actual resolver rejects a readable source of the wrong type"
    );
    let ledger = capture.finish()?;
    let input = ledger
        .sources
        .iter()
        .find(|source| source.role == ExportSourceRole::DerivedInput)
        .ok_or("failed input missing")?;
    assert_eq!(
        input.disposition,
        SourceDisposition::ProjectionFailed,
        "the existing canonical proof distinguishes typed rejection from failed reading"
    );
    let failure = ledger
        .derived_failures
        .first()
        .ok_or("retained-source failure missing")?;
    assert_eq!(
        failure.affected_kind,
        Kind::Items,
        "the secondary failure marks the retained original Item"
    );
    assert_eq!(
        failure.output_kind,
        Kind::Loadouts,
        "the failed output operation is explicitly identified"
    );
    assert_eq!(
        failure.record_index,
        record.index(),
        "the failure retains the actual physical source identity"
    );
    verify_original_projection(&original)?;
    Ok(())
}

#[test]
fn actual_canonical_root_failures_are_not_typed_projection_failures() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("EntityClassDefinition")
                .field("value", TestType::Int32)
                .declared_size(6),
        )
        .record(TestRecord::new(
            ROOT,
            "EntityClassDefinition",
            "EntityClassDefinition.Synthetic",
            "synthetic/root.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let capture = Arc::new(Capture::new(&ExportLedger::default(), None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone());
    assert!(
        loadout::read_with(
            &mut resolver,
            &mut PortReader::new(),
            &forge,
            Wanted {
                record_index: record.index(),
                guid: CatalogGuid(record.guid()),
                kind: EntityKind::Ship
            },
            &capture
        )
        .is_err(),
        "the canonical root layout really cannot be read"
    );
    let ledger = capture.finish()?;
    assert!(
        ledger
            .sources
            .iter()
            .all(|source| source.disposition == SourceDisposition::ReadFailed),
        "both actual dependency and root-input relationships preserve the read failure"
    );
    assert!(
        ledger.field_inspections.is_empty(),
        "no root proof is invented from the Wanted GUID"
    );
    Ok(())
}

#[test]
fn a_represented_unknown_attachment_is_information_and_keeps_the_row() -> TestResult {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(ROOT, "Root", "synthetic/root.xml")
                .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
                    "slot",
                    "UnavailableSyntheticClass",
                )]))
                .record(),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let tree = record.tree()?;
    let capture = Arc::new(Capture::new(&source(&record, &tree)?, None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone());
    let row = loadout::read_with(
        &mut resolver,
        &mut PortReader::new(),
        &forge,
        Wanted {
            record_index: record.index(),
            guid: CatalogGuid(record.guid()),
            kind: EntityKind::Item,
        },
        &capture,
    )
    .map_err(|failure| failure.reason)?;
    let row = row.ok_or("represented unresolved port row missing")?;
    assert_eq!(
        row.ports.first().ok_or("port missing")?.unresolved,
        Some("unknown_class"),
        "the unresolved attachment remains represented"
    );
    let ledger = capture.finish()?;
    let note = ledger
        .field_diagnostics
        .iter()
        .find(|diagnostic| diagnostic.warning_code.as_deref() == Some("unknown_class"))
        .ok_or("typed attachment information missing")?;
    assert_eq!(
        note.severity,
        ExportDiagnosticSeverity::Information,
        "represented source absence does not fabricate an unreadable-source warning"
    );
    assert!(
        ledger.derived_failures.is_empty(),
        "the successful represented projection is not a failed operation"
    );
    Ok(())
}

#[test]
fn real_field_decoder_errors_survive_optional_loadout_mapping() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("EntityClassDefinition").field("bad", TestType::Bool))
        .record(
            TestRecord::new(
                ROOT,
                "EntityClassDefinition",
                "EntityClassDefinition.Synthetic",
                "synthetic/root.xml",
            )
            .value("bad", TestValue::Raw(vec![2])),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.records().next().ok_or("source missing")?;
    let capture = Arc::new(Capture::new(&ExportLedger::default(), None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone());
    loadout::read_with(
        &mut resolver,
        &mut PortReader::new(),
        &forge,
        Wanted {
            record_index: record.index(),
            guid: CatalogGuid(record.guid()),
            kind: EntityKind::Item,
        },
        &capture,
    )
    .map_err(|failure| failure.reason)?;
    let ledger = capture.finish()?;
    assert!(
        ledger
            .field_inspections
            .iter()
            .flat_map(|inspection| &inspection.diagnostics)
            .any(
                |diagnostic| diagnostic.code == ExportDiagnosticCode::UnreadableSource
                    && diagnostic.severity == ExportDiagnosticSeverity::Error
            ),
        "an actual invalid bool leaf remains a decoder error even when optional mapping emits no row"
    );
    Ok(())
}

#[path = "represented_tests.rs"]
mod represented;

#[path = "file_tests.rs"]
mod files;

#[path = "alias_tests.rs"]
mod aliases;
#[path = "entry_tests.rs"]
mod entries;
#[path = "entry_cancel_tests.rs"]
mod entry_cancel;
#[path = "poison_tests.rs"]
mod poison;
