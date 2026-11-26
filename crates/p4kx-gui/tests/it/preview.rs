// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Text and Hex preview of the selected Entry, driven headless.

use crate::waiting::{RunQuiet, wait_for_model};
use std::error::Error;
use std::path::PathBuf;

use egui::OutputCommand;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::{Browser, Language, Model, PREVIEW_READ_LIMIT, PREVIEW_SIZE_LIMIT, Preview, ui};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn Error>>;

#[path = "preview_origin_tests.rs"]
mod origins;

const CONFIG: &str = "<config>\n  <ship name=\"Aurora\"/>\n</config>\n";
const BLOB: &[u8] = b"Hello\x00\x01\xff world, binary data!";

fn long_text() -> Vec<u8> {
    let line = b"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcde\n";
    line.iter()
        .copied()
        .cycle()
        .take(PREVIEW_READ_LIMIT + 100)
        .collect()
}

fn archive() -> Result<(TempDir, PathBuf), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Libs\\config.xml", CONFIG))
        .entry(TestEntry::deflated("Data\\deflated.txt", "deflated text"))
        .entry(TestEntry::zstd("Data\\zstd.txt", "zstd text"))
        .entry(TestEntry::stored("Data\\secret.txt", "encrypted text").encrypted())
        .entry(TestEntry::deflated("Data\\secret.def", "encrypted deflated text").encrypted())
        .entry(TestEntry::zstd("Data\\secret.zst", "encrypted zstd text").encrypted())
        .entry(TestEntry::stored("Data\\blob.bin", BLOB))
        .entry(TestEntry::stored("Data\\long.txt", long_text()))
        .entry(
            TestEntry::stored("Data\\huge.bin", vec![0; 16])
                .with_uncompressed_size(PREVIEW_SIZE_LIMIT * 2),
        )
        .entry(TestEntry::stored("Data\\cut.txt", vec![b'a'; 64]).with_payload(vec![b'a'; 8]))
        .entry(
            TestEntry::stored("Data\\limit.txt", vec![b'b'; 1])
                .with_uncompressed_size(PREVIEW_SIZE_LIMIT),
        )
        .write_to(&path)?;
    Ok((folder, path))
}

fn opened(language: Language) -> Result<(TempDir, Harness<'static, Model>), Box<dyn Error>> {
    let (folder, path) = archive()?;
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1600.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                ui::show(ui, model);
            },
            Model::new(language),
        );
    harness
        .state_mut()
        .set_archive_key(Some(p4kx_testkit::test_key()));
    harness.state_mut().open(path, || {});
    wait_until(&mut harness, |model| model.browser().is_some())?;
    Ok((folder, harness))
}

fn wait_until(harness: &mut Harness<'_, Model>, done: impl Fn(&Model) -> bool) -> TestResult {
    wait_for_model(harness, "the window to settle", done)?;
    harness.run_quiet();
    Ok(())
}

fn preview(harness: &mut Harness<'_, Model>, name: &str) -> TestResult {
    harness.get_by_label(name).click();
    wait_until(harness, |model| {
        model
            .browser()
            .and_then(Browser::preview)
            .is_some_and(|preview| !preview.is_loading())
    })
}

fn copied(harness: &mut Harness<'_, Model>) -> Option<String> {
    harness.get_by_label("Copy").click();
    harness.step();
    harness
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|command| match command {
            OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
}

fn hex_row(offset: usize, bytes: &[u8]) -> String {
    let hex: Vec<String> = bytes.iter().map(|byte| format!("{byte:02X}")).collect();
    let ascii: String = bytes
        .iter()
        .map(|&byte| {
            if byte.is_ascii_graphic() || byte == b' ' {
                char::from(byte)
            } else {
                '.'
            }
        })
        .collect();
    format!("{offset:08X}  {:<47}  {ascii}", hex.join(" "))
}

#[test]
fn nothing_is_previewed_until_an_entry_is_selected() -> TestResult {
    let (_folder, harness) = opened(Language::BritishEnglish)?;

    assert!(
        harness
            .query_by_label("Select an Entry to preview it.")
            .is_some(),
        "hint"
    );
    Ok(())
}

#[test]
fn selecting_a_text_entry_shows_its_lines_in_the_text_tab() -> TestResult {
    let (_folder, mut harness) = opened(Language::BritishEnglish)?;

    preview(&mut harness, "config.xml")?;

    assert!(
        harness
            .query_all_by_label("Data\\Libs\\config.xml")
            .next()
            .is_some(),
        "path"
    );
    for line in ["<config>", "  <ship name=\"Aurora\"/>", "</config>"] {
        assert!(harness.query_by_label(line).is_some(), "{line} is shown");
    }
    assert!(
        harness.query_by_label_contains("Truncated").is_none(),
        "complete"
    );
    Ok(())
}

#[test]
fn every_compression_method_and_encrypted_entries_are_previewed() -> TestResult {
    let (_folder, mut harness) = opened(Language::BritishEnglish)?;

    for (name, text) in [
        ("deflated.txt", "deflated text"),
        ("zstd.txt", "zstd text"),
        ("secret.txt", "encrypted text"),
        ("secret.def", "encrypted deflated text"),
        ("secret.zst", "encrypted zstd text"),
    ] {
        preview(&mut harness, name)?;

        assert!(harness.query_by_label(text).is_some(), "{name} is decoded");
    }
    Ok(())
}

#[test]
fn the_hex_tab_shows_offsets_bytes_and_printable_characters() -> TestResult {
    let (_folder, mut harness) = opened(Language::BritishEnglish)?;
    preview(&mut harness, "blob.bin")?;

    harness.get_by_label("Hex").click();
    harness.run_quiet();

    let (first, second) = BLOB.split_at(16);
    assert!(
        harness.query_by_label(&hex_row(0, first)).is_some(),
        "first row"
    );
    assert!(
        harness.query_by_label(&hex_row(16, second)).is_some(),
        "short last row"
    );
    Ok(())
}

#[test]
fn an_entry_above_the_size_limit_is_not_read() -> TestResult {
    let (_folder, mut harness) = opened(Language::BritishEnglish)?;

    preview(&mut harness, "huge.bin")?;

    assert!(
        harness
            .query_by_label(
                "Data\\huge.bin is 512.0 MiB. Entries larger than 256.0 MiB are not \
                 previewed; extract it to look inside."
            )
            .is_some(),
        "too large"
    );
    Ok(())
}

#[test]
fn an_entry_at_the_size_limit_is_still_read() -> TestResult {
    let (_folder, mut harness) = opened(Language::BritishEnglish)?;

    preview(&mut harness, "limit.txt")?;

    assert!(
        harness
            .query_by_label_contains("are not previewed")
            .is_none(),
        "not too large"
    );
    Ok(())
}

#[test]
fn a_long_entry_is_truncated_with_a_note() -> TestResult {
    let (_folder, mut harness) = opened(Language::BritishEnglish)?;

    preview(&mut harness, "long.txt")?;

    assert!(
        harness
            .query_by_label("Truncated: showing the first 1,048,576 of 1,048,676 bytes.")
            .is_some(),
        "note"
    );
    let shown = harness
        .state()
        .browser()
        .and_then(Browser::preview)
        .and_then(Preview::contents)
        .map(|contents| contents.bytes().len());
    assert_eq!(shown, Some(PREVIEW_READ_LIMIT), "only the limit is read");
    Ok(())
}

#[test]
fn an_unreadable_entry_names_the_reason() -> TestResult {
    let (_folder, mut harness) = opened(Language::BritishEnglish)?;

    preview(&mut harness, "cut.txt")?;

    assert!(
        harness
            .query_by_label_contains("Cannot read Data\\cut.txt: ")
            .is_some(),
        "failure"
    );
    Ok(())
}

#[test]
fn the_text_tab_copies_the_text() -> TestResult {
    let (_folder, mut harness) = opened(Language::BritishEnglish)?;
    preview(&mut harness, "config.xml")?;

    let text = copied(&mut harness);

    assert_eq!(text.as_deref(), Some(CONFIG), "the whole text");
    Ok(())
}

#[test]
fn the_hex_tab_copies_the_dump() -> TestResult {
    let (_folder, mut harness) = opened(Language::BritishEnglish)?;
    preview(&mut harness, "blob.bin")?;
    harness.get_by_label("Hex").click();
    harness.run_quiet();

    let text = copied(&mut harness);

    let (first, second) = BLOB.split_at(16);
    let dump = format!("{}\n{}\n", hex_row(0, first), hex_row(16, second));
    assert_eq!(text, Some(dump), "every row");
    Ok(())
}

#[test]
fn the_user_help_states_both_size_limits() -> TestResult {
    let help = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/user-help.md"
    ))?;
    let mib = |bytes: u64| format!("| {} MiB |", bytes / (1024 * 1024));

    assert!(
        help.contains(&mib(PREVIEW_SIZE_LIMIT)),
        "largest Entry previewed"
    );
    assert!(
        help.contains(&mib(u64::try_from(PREVIEW_READ_LIMIT)?)),
        "bytes shown"
    );
    Ok(())
}

#[test]
fn the_german_preview_notes_the_truncation() -> TestResult {
    let (_folder, mut harness) = opened(Language::German)?;

    preview(&mut harness, "long.txt")?;

    assert!(
        harness
            .query_by_label("Gekürzt: angezeigt werden die ersten 1.048.576 von 1.048.676 Bytes.")
            .is_some(),
        "German note"
    );
    Ok(())
}
