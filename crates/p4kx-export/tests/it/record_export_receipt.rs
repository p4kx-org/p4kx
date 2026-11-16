// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;

use p4kx_export::{
    CancelToken, ConflictPolicy, NDJSON_FILE, RecordExportOptions, RecordExportSummary,
    RecordFilePlans, RecordForm, write_planned_record_export,
    write_planned_record_export_with_receipt,
};
use p4kx_extraction::{ArchiveWriteProtection, DataForge};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

fn fixture() -> Vec<u8> {
    ["c", "a", "b"]
        .into_iter()
        .enumerate()
        .fold(
            DataForgeBuilder::new()
                .structure(TestStruct::new("Part").field("mass", TestType::Int32)),
            |file, (id, name)| {
                file.record(
                    TestRecord::new(
                        id as u128 + 1,
                        "Part",
                        format!("Part.{name}"),
                        format!("parts/{name}.xml"),
                    )
                    .value("mass", TestValue::Int32(7)),
                )
            },
        )
        .build()
}

fn receipt(summary: &RecordExportSummary) -> Result<&[u32], &'static str> {
    Ok(summary.receipt().ok_or("receipt requested")?.indices())
}

#[test]
fn single_file_receipts_cover_committed_records_and_exclude_existing_files() -> TestResult {
    let fixture = fixture();
    let forge = DataForge::open(&fixture)?;
    let records: Vec<_> = forge.records().collect();
    let plans = RecordFilePlans::default();
    for form in [RecordForm::Ndjson, RecordForm::FlatCsv] {
        let with = tempfile::tempdir()?;
        let plain = tempfile::tempdir()?;
        let options = RecordExportOptions::default();
        let written = write_planned_record_export_with_receipt(
            &forge,
            &plans,
            &records,
            form,
            with.path(),
            &options,
        );
        let unobserved =
            write_planned_record_export(&forge, &plans, &records, form, plain.path(), &options);
        assert_eq!(
            receipt(&written)?,
            &[1, 2, 0],
            "file order of the published definitions"
        );
        assert_eq!(
            written.written(),
            3,
            "all three definitions count as written"
        );
        assert!(
            written
                .receipt()
                .is_some_and(|receipt| receipt.rowless_indices().is_empty()),
            "every fixture definition emits a row"
        );
        assert!(unobserved.receipt().is_none(), "opt-in tracking");
        let file = form.file().ok_or("single-file form")?;
        let bytes = fs::read(with.path().join(file))?;
        assert_eq!(
            bytes,
            fs::read(plain.path().join(file))?,
            "same payload with receipts"
        );
        for policy in [ConflictPolicy::Skip, ConflictPolicy::IfChanged] {
            let kept = write_planned_record_export_with_receipt(
                &forge,
                &plans,
                &records,
                form,
                with.path(),
                &RecordExportOptions { policy, ..options },
            );
            assert_eq!(kept.written(), 0, "existing file under {policy:?}");
            assert!(receipt(&kept)?.is_empty(), "no committed definitions");
            assert_eq!(fs::read(with.path().join(file))?, bytes, "kept file");
        }
    }
    Ok(())
}

#[test]
fn a_definition_counted_without_a_flat_value_is_marked_rowless() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Empty"))
        .record(TestRecord::new(1, "Empty", "Empty.Record", "empty.xml"))
        .build();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let target = tempfile::tempdir()?;
    let summary = write_planned_record_export_with_receipt(
        &forge,
        &RecordFilePlans::default(),
        &records,
        RecordForm::FlatNdjson,
        target.path(),
        &RecordExportOptions::default(),
    );
    assert_eq!(
        summary.written(),
        1,
        "the existing counter includes rowless Records"
    );
    let receipt = summary.receipt().ok_or("receipt requested")?;
    assert_eq!(
        receipt.indices(),
        &[0],
        "the count has the exact original definition"
    );
    assert_eq!(
        receipt.rowless_indices(),
        &[0],
        "the definition supplied no data row"
    );
    let file = RecordForm::FlatNdjson.file().ok_or("flat path")?;
    assert_eq!(fs::read(target.path().join(file))?, b"", "rowless payload");
    Ok(())
}

#[test]
fn a_successful_empty_single_file_has_an_empty_receipt() -> TestResult {
    let fixture = fixture();
    let forge = DataForge::open(&fixture)?;
    let target = tempfile::tempdir()?;
    let summary = write_planned_record_export_with_receipt(
        &forge,
        &RecordFilePlans::default(),
        &[],
        RecordForm::Ndjson,
        target.path(),
        &RecordExportOptions::default(),
    );
    assert!(summary.is_complete(), "empty result committed");
    assert_eq!(
        fs::read(target.path().join(NDJSON_FILE))?,
        b"",
        "empty NDJSON"
    );
    assert!(
        receipt(&summary)?.is_empty(),
        "no original definitions were written"
    );
    Ok(())
}

#[test]
fn interruption_before_single_file_publication_discards_staged_positions() -> TestResult {
    let fixture = fixture();
    let forge = DataForge::open(&fixture)?;
    let records: Vec<_> = forge.records().collect();
    let target = tempfile::tempdir()?;
    let source = target.path().join("source.p4k");
    fs::write(&source, b"source")?;
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(&source, cancel.flag())?;
    let progress = || cancel.cancel();
    let options = RecordExportOptions {
        protection: Some(&protection),
        cancel: Some(&cancel),
        progress: Some(&progress),
        ..RecordExportOptions::default()
    };
    let summary = write_planned_record_export_with_receipt(
        &forge,
        &RecordFilePlans::default(),
        &records,
        RecordForm::Ndjson,
        target.path(),
        &options,
    );
    assert!(
        cancel.is_cancelled(),
        "work was prepared before cancellation"
    );
    assert_eq!(summary.written(), 0, "no file was committed");
    assert!(
        receipt(&summary)?.is_empty(),
        "pending positions were discarded"
    );
    assert!(
        !target.path().join(NDJSON_FILE).exists(),
        "no partial public file"
    );
    Ok(())
}

#[test]
fn partial_per_file_export_keeps_the_receipt_of_the_file_already_written() -> TestResult {
    let fixture = fixture();
    let forge = DataForge::open(&fixture)?;
    let records: Vec<_> = forge.records().collect();
    let target = tempfile::tempdir()?;
    let cancel = CancelToken::new();
    let progress = || cancel.cancel();
    let options = RecordExportOptions {
        cancel: Some(&cancel),
        progress: Some(&progress),
        ..RecordExportOptions::default()
    };
    let one_worker = rayon::ThreadPoolBuilder::new().num_threads(1).build()?;
    let summary = one_worker.install(|| {
        write_planned_record_export_with_receipt(
            &forge,
            &RecordFilePlans::default(),
            &records,
            RecordForm::Json,
            target.path(),
            &options,
        )
    });
    assert_eq!(summary.written(), 1, "first file committed");
    assert_eq!(summary.cancelled(), 2, "remaining files stopped");
    let [index] = receipt(&summary)? else {
        return Err("one committed definition expected".into());
    };
    let record = records.get(*index as usize).ok_or("committed definition")?;
    let file = format!("{}.json", record.record_path()?);
    assert!(
        target.path().join(file).is_file(),
        "the receipt names the committed Record"
    );
    assert_eq!(
        fs::read_dir(target.path().join("parts"))?.count(),
        1,
        "only one output file"
    );
    Ok(())
}
