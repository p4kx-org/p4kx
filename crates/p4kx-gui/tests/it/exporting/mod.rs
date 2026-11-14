// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Helpers of the Record export tests: a synthetic Archive with a DataForge, opened headless.

use crate::waiting::wait_polling;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_gui::{Browser, Language, Model, RecordExport, RecordExportStatus, RecordTreeStatus};
use p4kx_testkit::TestValue;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType};
use tempfile::TempDir;

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

/// Two ships, one of them referring to a Record that does not exist, and two weapons sharing a
/// record path.
pub(crate) fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("EntityClassDefinition")
                .field("mass", TestType::Int32)
                .field("maker", TestType::Reference),
        )
        .structure(TestStruct::new("AmmoParams").field("speed", TestType::Int32))
        .record(
            TestRecord::new(
                1,
                "EntityClassDefinition",
                "EntityClassDefinition.AEGS_Avenger",
                "libs/foundry/records/entities/spaceships/aegs_avenger.xml",
            )
            .value("mass", TestValue::Int32(25_000))
            .value("maker", TestValue::Reference(0x77)),
        )
        .record(
            TestRecord::new(
                2,
                "EntityClassDefinition",
                "EntityClassDefinition.ANVL_Arrow",
                "libs/foundry/records/entities/spaceships/anvl_arrow.xml",
            )
            .value("mass", TestValue::Int32(12_000))
            .value("maker", TestValue::Reference(1)),
        )
        .record(
            TestRecord::new(
                3,
                "AmmoParams",
                "AmmoParams.Laser_S1",
                "libs/foundry/records/weapons.xml",
            )
            .value("speed", TestValue::Int32(1200)),
        )
        .record(
            TestRecord::new(
                4,
                "AmmoParams",
                "AmmoParams.Ballistic_S1",
                "libs/foundry/records/weapons.xml",
            )
            .value("speed", TestValue::Int32(900)),
        )
}

/// `count` items, each at its own record path, enough to keep an export busy for a while.
pub(crate) fn many(count: u32) -> DataForgeBuilder {
    (0..count).fold(
        DataForgeBuilder::new().structure(TestStruct::new("Item").field("count", TestType::Int32)),
        |builder, number| {
            builder.record(
                TestRecord::new(
                    u128::from(number) + 1,
                    "Item",
                    format!("Item.N{number:05}"),
                    format!("libs/foundry/records/items/n{number:05}.xml"),
                )
                .value(
                    "count",
                    TestValue::Int32(i32::try_from(number).unwrap_or_default()),
                ),
            )
        },
    )
}

/// Writes an Archive holding `forge` as `Data\Game2.dcb`, opens it and creates the empty folder
/// `out` next to it.
pub(crate) fn opened(
    forge: &DataForgeBuilder,
) -> Result<(TempDir, Model, PathBuf), Box<dyn Error>> {
    opened_with(
        &ArchiveBuilder::new()
            .entry(TestEntry::stored("Data\\Libs\\config.xml", "config"))
            .entry(TestEntry::zstd("Data\\Game2.dcb", forge.build())),
    )
}

/// Writes the Archive of `builder`, opens it and creates the empty folder `out` next to it.
pub(crate) fn opened_with(
    builder: &ArchiveBuilder,
) -> Result<(TempDir, Model, PathBuf), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("Data.p4k");
    builder.write_to(&path)?;
    let mut model = Model::new(Language::BritishEnglish);
    model.open(path, || {});
    wait_until(&mut model, |model| model.browser().is_some())?;
    let out = dir.path().join("out");
    fs::create_dir(&out)?;
    Ok((dir, model, out))
}

/// Polls `model` until `done` holds.
pub(crate) fn wait_until(model: &mut Model, done: impl Fn(&Model) -> bool) -> TestResult {
    wait_polling(
        model,
        "the window to settle",
        |model| {
            model.poll();
            model.poll_record_export();
            if let Some(browser) = model.browser_mut() {
                browser.poll();
            }
        },
        done,
    )?;
    Ok(())
}

/// Loads DataForge and selects the Record tree node at `path`.
pub(crate) fn select_node(model: &mut Model, path: &str) -> TestResult {
    let browser = model.browser_mut().ok_or("no Archive")?;
    browser.load_records(|| {});
    wait_until(model, |model| {
        model
            .browser()
            .and_then(Browser::records)
            .is_some_and(|tree| matches!(tree.status(), RecordTreeStatus::Ready(_)))
    })?;
    let browser = model.browser_mut().ok_or("no Archive")?;
    let node = browser
        .records()
        .and_then(|tree| tree.find(path))
        .ok_or_else(|| format!("no node {path}"))?;
    browser.select_record_node(node, || {});
    Ok(())
}

/// Waits until the Record export ended and returns where it stands.
pub(crate) fn ended(model: &mut Model) -> Result<RecordExportStatus<'_>, Box<dyn Error>> {
    wait_until(model, |model| {
        model.record_export().is_some_and(|run| !run.is_running())
    })?;
    model
        .record_export()
        .map(RecordExport::status)
        .ok_or_else(|| "no Record export".into())
}

/// Returns the files below `dir` as sorted paths relative to it with `/` separators.
pub(crate) fn files_below(dir: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(folder) = pending.pop() {
        for item in fs::read_dir(folder)? {
            let path = item?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.push(path.strip_prefix(dir)?.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    files.sort();
    Ok(files)
}
pub(crate) fn wait_for_writing(model: &mut Model) -> TestResult {
    wait_until(model, |model| {
        model.record_export().is_some_and(|run| {
            matches!(
                run.status(),
                RecordExportStatus::Running(progress)
                    if progress.stage == p4kx_gui::RecordExportStage::Writing
            )
        })
    })
}
