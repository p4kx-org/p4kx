// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_media::ExportOptions;

use super::layout_fixture;
use super::layout_support::{field, row};
use super::support::{self, TestResult};

#[test]
fn a_changed_layout_sidecar_is_not_accepted_as_a_complete_existing_generation() -> TestResult {
    let folder = tempfile::tempdir()?;
    let fixture = layout_fixture::zip(&[])?;
    let archive = layout_fixture::open(folder.path(), &fixture.bytes, false)?;
    let output = folder.path().join("media");
    let index = support::published(p4kx_media::export(
        &archive,
        &output,
        &ExportOptions::default(),
    )?)?;
    let rows = support::records(&output, &index, "archive.jsonl")?;
    let chunk = row(&rows, "archive-raw-chunk")?;
    let relative = field(&chunk.data, "/chunk/artifact/path")?
        .as_str()
        .ok_or("sidecar path")?;
    let path = output.join(&index.artifact_base).join(relative);
    let original = fs::read(&path)?;
    let mut changed = original.clone();
    *changed.first_mut().ok_or("nonempty directory sidecar")? ^= 0xff;
    fs::write(&path, changed)?;
    let old_index = fs::read(output.join("media-index.json"))?;
    assert!(
        p4kx_media::export(&archive, &output, &ExportOptions::default()).is_err(),
        "existing-generation validation verifies layout sidecars from the common artifact manifest"
    );
    assert_eq!(fs::read(output.join("media-index.json"))?, old_index);
    assert!(
        original.starts_with(b"PK\x01\x02"),
        "the corrupted artifact was source metadata rather than a payload"
    );
    Ok(())
}
