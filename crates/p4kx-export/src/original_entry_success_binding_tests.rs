// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};

use super::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_read::original::{EntryCapture, EntryRead, EntryReadError};
use crate::entry_source::EntrySource;
use crate::export_health_compile;
use crate::export_health_prepared::PreparedHealth;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::texts::TextSources;
use crate::{
    CatalogError, CatalogPublicationMode, EntryOperation, ExportAccountingError, Kind, KindSummary,
};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
const PAYLOAD: &[u8] = b"actual successful source";

fn with_inputs(
    check: impl FnOnce(
        &Archive,
        &OriginalCatalogInput<'_, '_>,
        &OriginalCatalogInput<'_, '_>,
    ) -> TestResult,
) -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Other"))
        .record(TestRecord::new(1, "Other", "Source", "source.xml"))
        .build();
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", bytes))
        .entry(TestEntry::stored("entry-source.bin", PAYLOAD))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let texts = TextSources::read(&archive)?;
    let first = CatalogOrigins::new(&archive, &entry, &texts)?;
    let second = CatalogOrigins::new(&archive, &entry, &texts)?;
    assert!(
        !first.uses.is_same_inventory(&second.uses),
        "two genuine report preparations have independent inventories"
    );
    let first = OriginalCatalogInput::new(&forge, &first);
    let second = OriginalCatalogInput::new(&forge, &second);
    check(&archive, &first, &second)
}

fn source(
    archive: &Archive,
    input: &OriginalCatalogInput<'_, '_>,
    failed: bool,
) -> TestResult<EntrySource> {
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == "entry-source.bin")
        .ok_or("the genuine original Entry")?;
    let capture = EntryCapture::for_input(input);
    capture.check_archive(archive.input_origin())?;
    let source = EntrySource::outer(&capture, archive, entry)?.operation(EntryOperation::SourceXml);
    source.receipt().check_input(input)?;
    let outcome = EntryRead::read(
        source.receipt().clone(),
        archive,
        entry,
        if failed { 0 } else { 1024 },
    )?;
    if failed {
        assert!(
            matches!(outcome, Err(EntryReadError::Limit { limit: 0 })),
            "the independent zero-budget read really fails before payload admission"
        );
    } else {
        let read = outcome.map_err(|_| "the original complete Entry read must succeed")?;
        assert_eq!(
            read.bytes(),
            PAYLOAD,
            "the actual source bytes were read in full before recording success"
        );
    }
    Ok(source)
}

fn prepared(source: EntrySource, disposition: SourceDisposition) -> TestResult<PreparedHealth> {
    let mut ledger = ExportLedger::default();
    for kind in Kind::ALL {
        ledger.inventory(kind);
    }
    source.record(&mut ledger, Kind::LootContainers, disposition);
    let rows: Vec<_> = Kind::ALL
        .into_iter()
        .map(|kind| KindSummary {
            kind,
            count: 0,
            skipped: 0,
            complete: true,
        })
        .collect();
    Ok(export_health_compile::prepare_report(
        ledger,
        &rows,
        [],
        &[],
        CatalogPublicationMode::Normal,
    )?)
}

#[test]
fn genuine_successful_entry_cannot_admit_another_report_inventory() -> TestResult {
    with_inputs(|archive, first, second| {
        let source = source(archive, first, false)?;
        assert!(
            matches!(
                source.receipt().check_input(second),
                Err(ExportAccountingError::ConflictingSourceDatum)
            ),
            "the existing original receipt independently proves that the second inventory is foreign"
        );
        let prepared = prepared(source, SourceDisposition::Projected)?;
        assert_eq!(
            prepared
                .kind(Kind::LootContainers)
                .ok_or("actual source kind")?
                .source_entries
                .projected,
            1,
            "the authentic successful operation reached final accounting"
        );
        assert!(
            prepared.failed_entries.is_empty(),
            "no retained failure can incidentally catch this boundary"
        );
        let before = second.origins().uses.freeze()?;
        let outcome = prepared.finish(second, &[], None);
        assert!(
            matches!(
                outcome,
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "successful original Entry operations must also belong to the final report inventory"
        );
        assert_eq!(
            second.origins().uses.freeze()?,
            before,
            "refusal contributes no foreign source uses"
        );
        Ok(())
    })
}

#[test]
fn genuine_successful_entry_of_this_report_retains_counts_without_failure_text() -> TestResult {
    with_inputs(|archive, input, _| {
        let prepared = prepared(source(archive, input, false)?, SourceDisposition::Projected)?;
        let report = prepared.finish(input, &[], None)?;
        assert_eq!(
            report
                .kind(Kind::LootContainers)
                .ok_or("actual source kind")?
                .source_entries
                .projected,
            1,
            "the authentic original success remains one projected Entry"
        );
        assert!(
            report.failed_entries.is_empty(),
            "successful operations create no failure body"
        );
        assert!(
            report.metadata().texts().is_empty(),
            "unretained successful paths add no sealed Health text atoms"
        );
        Ok(())
    })
}

#[test]
fn failed_entry_rejects_the_same_foreign_inventory() -> TestResult {
    with_inputs(|archive, first, second| {
        let source = source(archive, first, true)?;
        assert!(
            matches!(
                source.receipt().check_input(second),
                Err(ExportAccountingError::ConflictingSourceDatum)
            ),
            "the failure uses the same independently foreign original receipt"
        );
        let prepared = prepared(source, SourceDisposition::ReadFailed)?;
        assert_eq!(
            prepared.failed_entries.len(),
            1,
            "the genuine failed read has one retained failure"
        );
        let outcome = prepared.finish(second, &[], None);
        assert!(
            matches!(
                outcome,
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "retained failures already apply the final input guard"
        );
        Ok(())
    })
}
