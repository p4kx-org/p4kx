// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use super::{TestResult, fixture, run};

#[test]
fn combined_export_cannot_replace_its_localization_archive() -> TestResult {
    let fixture = fixture()?;
    let text_archive = fixture.root.join("Game2.xml");
    fs::copy(&fixture.archive, &text_archive)?;
    let original = fs::read(&text_archive)?;
    let message = run(
        &["export"],
        &fixture.file,
        &[
            "--combined",
            "--lang",
            "en",
            "--archive",
            text_archive.to_str().ok_or("language Archive path")?,
            "--conflict",
            "overwrite",
            "--out",
            fixture.root.to_str().ok_or("output folder path")?,
        ],
        2,
    )?;
    assert!(
        message.contains("[p4kx.write.source-overwrite]"),
        "the language Archive must trigger the source guard: {message}"
    );
    assert_eq!(fs::read(&text_archive)?, original, "original Archive bytes");
    assert!(
        !text_archive
            .with_file_name("Game2.xml.p4kx-origin.jsonl")
            .exists(),
        "a refused export cannot publish a companion"
    );
    Ok(())
}
