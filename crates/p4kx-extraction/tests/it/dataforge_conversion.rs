// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Conversion of the DataForge Entry into Record XML files during an Extraction.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use p4kx_extraction::{
    Archive, CancelToken, ConflictPolicy, ExtractOptions, ExtractionError, ExtractionReport,
    Progress, extract,
};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, SplitDds, TestElement,
    TestEntry, TestRecord, TestStruct, TestTextureFormat, TestType, TestValue,
};

type TestResult = Result<(), Box<dyn Error>>;

fn database(count: u32) -> Vec<u8> {
    (0..count)
        .fold(
            DataForgeBuilder::new().structure(
                TestStruct::new("Item")
                    .field("count", TestType::Int32)
                    .field("label", TestType::String),
            ),
            |builder, number| {
                let record = TestRecord::new(
                    u128::from(number) + 1,
                    "Item",
                    format!("Item.N{number}"),
                    format!("libs/foundry/records/items/n{number}.xml"),
                )
                .value("count", TestValue::Int32(7));
                builder.record(if number == 0 {
                    record.value("label", TestValue::Raw(vec![0xff; 4]))
                } else {
                    record.value("label", TestValue::text("fine"))
                })
            },
        )
        .build()
}

struct Fixture {
    _dir: tempfile::TempDir,
    archive: Archive,
    target: PathBuf,
}

fn fixture(entries: Vec<TestEntry>) -> Result<Fixture, Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    let target = dir.path().join("out");
    fs::create_dir(&target)?;
    Ok(Fixture {
        archive: Archive::open_with_key(&path, Some(p4kx_testkit::test_key()))?,
        target,
        _dir: dir,
    })
}

fn standard(count: u32) -> Result<Fixture, Box<dyn Error>> {
    fixture(vec![
        TestEntry::zstd("Data\\Game2.dcb", database(count)).encrypted(),
        TestEntry::stored("Data\\readme.txt", b"hello".to_vec()),
    ])
}

fn run(fixture: &Fixture, options: &ExtractOptions<'_>) -> ExtractionReport {
    let entries: Vec<_> = fixture.archive.entries().iter().collect();
    extract(&fixture.archive, &entries, &fixture.target, options)
}

fn converting() -> ExtractOptions<'static> {
    ExtractOptions {
        convert: true,
        ..ExtractOptions::default()
    }
}

fn files(root: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut found = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if path.is_dir() {
                folders.push(path);
            } else {
                found.push(
                    path.strip_prefix(root)?
                        .display()
                        .to_string()
                        .replace('\\', "/"),
                );
            }
        }
    }
    found.sort();
    Ok(found)
}

#[test]
fn conversion_writes_one_record_file_per_record_instead_of_the_dcb() -> TestResult {
    let fixture = standard(3)?;

    let report = run(&fixture, &converting());

    assert!(report.is_complete(), "complete: {report:?}");
    assert_eq!(report.extracted(), 2, "both Entries count: {report:?}");
    assert_eq!(report.converted(), Some(1), "the DataForge is converted");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/libs/foundry/records/items/n0.xml",
            "Data/libs/foundry/records/items/n1.xml",
            "Data/libs/foundry/records/items/n2.xml",
            "Data/readme.txt",
            "Data/readme.txt.p4kx-origin.jsonl",
        ],
        "the Records go below the folder of the Entry, without the .dcb"
    );
    let xml = fs::read_to_string(
        fixture
            .target
            .join("Data/libs/foundry/records/items/n1.xml"),
    )?;
    assert_eq!(
        xml,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <Item xmlns:p4kx=\"https://github.com/p4kx-org/p4kx/record-xml\" \
         guid=\"00000000-0000-0000-0000-000000000002\" \
         path=\"libs/foundry/records/items/n1\" p4kx:name=\"Item.N1\" count=\"7\" label=\"fine\"/>\n",
        "Record XML"
    );
    Ok(())
}

#[test]
fn records_with_error_values_are_written_and_counted() -> TestResult {
    let fixture = standard(3)?;

    let report = run(&fixture, &converting());

    let records = report.records().ok_or("the Records are reported")?;
    assert_eq!(records.written(), 3, "every Record is written: {records:?}");
    assert_eq!(records.error_values(), 1, "the broken label is counted");
    assert_eq!(records.records_with_errors(), 1, "in one Record");
    let broken = fs::read_to_string(
        fixture
            .target
            .join("Data/libs/foundry/records/items/n0.xml"),
    )?;
    assert!(broken.contains("<p4kx:error field=\"label\""), "{broken}");
    Ok(())
}

#[test]
fn without_conversion_the_dcb_is_written_as_before() -> TestResult {
    let fixture = standard(2)?;

    let report = run(&fixture, &ExtractOptions::default());

    assert!(report.records().is_none(), "no Records: {report:?}");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Game2.dcb",
            "Data/Game2.dcb.p4kx-origin.jsonl",
            "Data/readme.txt",
            "Data/readme.txt.p4kx-origin.jsonl",
        ],
        "raw Entries"
    );
    assert_eq!(
        fs::read(fixture.target.join("Data/Game2.dcb"))?,
        database(2),
        "unchanged bytes"
    );
    Ok(())
}

#[test]
fn only_the_dataforge_the_archive_is_read_from_is_converted() -> TestResult {
    let fixture = fixture(vec![
        TestEntry::stored("Data\\Game.dcb", database(1)),
        TestEntry::stored("Data\\game2.DCB", database(2)),
    ])?;

    let report = run(&fixture, &converting());

    assert_eq!(report.converted(), Some(1), "one DataForge: {report:?}");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Game.dcb",
            "Data/Game.dcb.p4kx-origin.jsonl",
            "Data/libs/foundry/records/items/n0.xml",
            "Data/libs/foundry/records/items/n1.xml",
        ],
        "Game2.dcb is converted, Game.dcb is written as it is"
    );
    Ok(())
}

#[test]
fn a_dataforge_that_cannot_be_opened_fails_the_entry() -> TestResult {
    let mut corrupt = database(1);
    corrupt.truncate(12);
    let fixture = fixture(vec![TestEntry::stored("Data\\Game2.dcb", corrupt)])?;

    let report = run(&fixture, &converting());

    assert!(
        matches!(
            report.failures(),
            [failure] if failure.path() == "Data\\Game2.dcb"
                && matches!(failure.error(), ExtractionError::DataForge(_))
        ),
        "the Entry fails: {report:?}"
    );
    assert!(files(&fixture.target)?.is_empty(), "nothing is written");
    Ok(())
}

#[path = "dataforge_conversion_checksum.rs"]
mod checksum;
#[path = "dataforge_conversion_extent.rs"]
mod extent;

#[test]
fn the_conflict_policy_applies_to_each_record_file() -> TestResult {
    let fixture = standard(2)?;
    run(&fixture, &converting());
    let file = fixture
        .target
        .join("Data/libs/foundry/records/items/n1.xml");
    fs::write(&file, "changed")?;

    let skip = run(&fixture, &converting());
    let kept = fs::read(&file)?;
    let overwrite = run(
        &fixture,
        &ExtractOptions {
            policy: ConflictPolicy::Overwrite,
            ..converting()
        },
    );

    assert_eq!(kept, b"changed", "skip keeps the Record file");
    let skipped = skip.records().map(|records| records.skipped().to_vec());
    assert_eq!(
        skipped,
        Some(vec![
            "libs/foundry/records/items/n0.xml".to_owned(),
            "libs/foundry/records/items/n1.xml".to_owned(),
        ]),
        "both Record files are kept"
    );
    assert_eq!(
        overwrite
            .records()
            .map(p4kx_extraction::RecordExportReport::written),
        Some(2),
        "rewritten"
    );
    assert_ne!(
        fs::read(&file)?,
        b"changed",
        "overwrite replaces the Record file"
    );
    Ok(())
}

#[test]
fn progress_advances_while_records_are_written_and_ends_complete() -> TestResult {
    let fixture = standard(400)?;
    let seen = Mutex::new(Vec::new());
    let on_progress = |progress: Progress| {
        if let Ok(mut seen) = seen.lock() {
            seen.push(progress);
        }
    };
    let options = ExtractOptions {
        progress: Some(&on_progress),
        ..converting()
    };

    let report = run(&fixture, &options);

    assert!(report.is_complete(), "complete: {report:?}");
    let seen = seen.into_inner().map_err(|_| "progress lock")?;
    let last = seen.last().ok_or("progress is reported")?;
    assert_eq!(last.entries_done, 2, "both Entries ended");
    assert_eq!(last.bytes_done, last.bytes_total, "every byte is counted");
    let during = seen
        .iter()
        .filter(|progress| progress.entries_done == 1 && progress.bytes_done < progress.bytes_total)
        .count();
    assert!(
        during >= 100,
        "the Records move the progress: {during} updates"
    );
    assert!(
        seen.windows(2)
            .all(|pair| pair[0].bytes_done <= pair[1].bytes_done),
        "progress never goes back"
    );
    Ok(())
}

#[test]
fn cancelling_while_records_are_written_stops_the_run() -> TestResult {
    let fixture = fixture(vec![TestEntry::stored("Data\\Game2.dcb", database(3000))])?;
    let cancel = CancelToken::new();
    let on_progress = |progress: Progress| {
        if progress.bytes_done * 2 > progress.bytes_total {
            cancel.cancel();
        }
    };
    let options = ExtractOptions {
        cancel: Some(&cancel),
        progress: Some(&on_progress),
        ..converting()
    };

    let report = run(&fixture, &options);

    assert_eq!(
        report.cancelled(),
        ["Data\\Game2.dcb"],
        "the Entry is cancelled: {report:?}"
    );
    let records = report.records().ok_or("the Records are reported")?;
    assert!(
        records.cancelled() > 0,
        "Records are cancelled: {records:?}"
    );
    assert!(
        records.written() < 3000,
        "not every Record is written: {records:?}"
    );
    assert!(!report.is_complete(), "a cancelled run is not complete");
    assert!(
        files(&fixture.target)?.iter().all(|file| Path::new(file)
            .extension()
            .is_some_and(|extension| extension == "xml")),
        "no partial file remains"
    );
    Ok(())
}

#[test]
fn cryxml_textures_and_the_dataforge_convert_in_one_run() -> TestResult {
    let cryxml = TestElement::new("Ship")
        .attribute("name", "Gladius")
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let mut entries = SplitDds::new(TestTextureFormat::LegacyDxt5, 32, 16)
        .split(3)
        .entries("Data\\Textures\\rock_diff.dds");
    entries.push(TestEntry::stored("Data\\ship.xml", cryxml));
    entries.push(TestEntry::stored("Data\\Game2.dcb", database(2)));
    let fixture = fixture(entries)?;

    let report = run(&fixture, &converting());

    assert!(report.is_complete(), "complete: {report:?}");
    assert_eq!(
        files(&fixture.target)?,
        [
            "Data/Textures/rock_diff.dds",
            "Data/Textures/rock_diff.dds.p4kx-origin.jsonl",
            "Data/Textures/rock_diff.png",
            "Data/libs/foundry/records/items/n0.xml",
            "Data/libs/foundry/records/items/n1.xml",
            "Data/ship.xml",
        ],
        "the texture merged and exported, the Records written, the CryXml converted"
    );
    assert!(
        fs::read(fixture.target.join("Data/ship.xml"))?.starts_with(b"<?xml"),
        "CryXml as XML"
    );
    assert_eq!(
        report
            .records()
            .map(p4kx_extraction::RecordExportReport::written),
        Some(2),
        "both Records"
    );
    Ok(())
}
