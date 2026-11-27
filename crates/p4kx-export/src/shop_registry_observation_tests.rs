// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::ArchiveFiles;
use p4kx_testkit::TestEntry;

use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_entry_ledger::EntryOperation;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_registry::RegistryNeed;
use crate::{Kind, ShopSummary};

use super::{CONTAINER, REGISTRY, TestResult, opened, read_shops, registry};

#[test]
fn an_absent_registry_without_starmap_is_explicitly_optional() -> TestResult {
    let (_folder, archive) = opened(Vec::new())?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        read_shops(&archive, capture)
    })?;
    assert_eq!(
        read.ledger.inventories,
        [Kind::Shops],
        "the source inventory completed"
    );
    assert_eq!(
        read.ledger.incomplete_inventories,
        Vec::new(),
        "optional absence hides no required inputs"
    );
    let [source] = read.ledger.entries.as_slice() else {
        return Err("one optional registry observation".into());
    };
    assert_eq!(source.entry_index, None, "no physical Entry is invented");
    assert_eq!(
        source.disposition,
        SourceDisposition::SkippedBeforeRead,
        "absence is not a blocked read"
    );
    assert_eq!(
        source.diagnosis.code,
        Some(ExportDiagnosticCode::OptionalAbsent),
        "the optional state is explicit"
    );
    Ok(())
}

#[test]
fn an_absent_registry_required_by_starmap_is_a_failed_configured_source() -> TestResult {
    let (_folder, archive) = opened(Vec::new())?;
    let mut summary = ShopSummary::default();
    let mut ledger = ExportLedger::default();
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        crate::shop_registry::read_source(
            &archive,
            &ArchiveFiles::new(&archive),
            RegistryNeed::Required,
            &mut summary,
            &mut ledger,
            capture,
        )
    })?;
    assert!(read.is_none(), "the configured registry is absent");
    let [source] = ledger.entries.as_slice() else {
        return Err("one required missing registry observation".into());
    };
    assert_eq!(
        ledger.incomplete_inventories,
        [Kind::Shops],
        "a required missing registry hides its configured shop inputs"
    );
    assert_eq!(
        source.entry_index, None,
        "a missing registry has no physical Entry"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::ReadFailed,
        "the actual consuming context requires this source"
    );
    assert_eq!(
        source.diagnosis.code,
        Some(ExportDiagnosticCode::DanglingReference),
        "required absence is distinguishable from optional absence"
    );
    Ok(())
}

#[test]
fn a_registry_projection_failure_does_not_claim_unconsumed_container_reads() -> TestResult {
    let (_folder, archive) = opened(vec![
        TestEntry::stored("padding.txt", "ignored"),
        TestEntry::stored(REGISTRY, "malformed XML"),
        TestEntry::stored(CONTAINER, "bad unused container"),
    ])?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        read_shops(&archive, capture)
    })?;
    assert_eq!(
        read.ledger.incomplete_inventories,
        [Kind::Shops],
        "unparseable registry bytes cannot enumerate shops"
    );
    let [source] = read.ledger.entries.as_slice() else {
        return Err("only the actually consumed registry".into());
    };
    assert_eq!(
        source.entry_index,
        Some(1),
        "the stored Entry index survives"
    );
    assert_eq!(
        source.operation,
        EntryOperation::ShopRegistry,
        "the failing operation survives"
    );
    assert_eq!(
        source.disposition,
        SourceDisposition::ProjectionFailed,
        "bytes were read but XML projection failed"
    );
    Ok(())
}

#[test]
fn malformed_registry_rows_retain_valid_shops_and_one_failed_source() -> TestResult {
    let malformed = r#"<ShopBackup ID="not-a-guid" SuperName="Root,Lost"/>"#;
    let (_folder, archive) = opened(vec![TestEntry::stored(REGISTRY, registry(malformed))])?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        read_shops(&archive, capture)
    })?;
    assert_eq!(
        read.shops.len(),
        1,
        "the valid registry row remains present"
    );
    let shop = read.shops.first().ok_or("one valid shop")?;
    let row = serde_json::to_value(shop)?;
    assert_eq!(
        row.pointer("/itemGuids/state")
            .and_then(serde_json::Value::as_str),
        Some("unreadable"),
        "the unresolved inventory retains its read status"
    );
    assert_eq!(
        row.pointer("/itemGuids/value"),
        Some(&serde_json::Value::Null),
        "the unresolved inventory exports a null item list"
    );
    assert_eq!(
        read.ledger.incomplete_inventories,
        Vec::new(),
        "malformed enumerated rows do not imply a truncated source walk"
    );
    let entries = crate::export_health_entries::compile(read.ledger.entries)?;
    assert_eq!(
        entries.failed.len(),
        1,
        "failed source identity is independent of retained rows"
    );
    let counts = crate::export_health_entries::counts(Kind::Shops, &entries.sources)?;
    assert_eq!(counts.selected, 1, "the registry is one source Entry");
    assert_eq!(
        counts.failed_projections, 1,
        "the partial registry is not declared complete"
    );
    assert_eq!(
        counts.read_successful, 1,
        "the canonical source read succeeded"
    );
    Ok(())
}
