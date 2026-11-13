// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_media::{ExportOptions, ExportOutcome, MediaRecord, Status};
use p4kx_testkit::TestChunkFile;
use serde_json::Value;

use super::{TestResult, archive, options, validate_rows};

#[test]
fn an_unrecognized_global_image_stays_diagnostic_and_preserves_its_chunk() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let image = TestChunkFile::ivo()
        .chunk(0x1234, 0x902, b"unknown")
        .bytes();
    let directional = TestChunkFile::crch().chunk(0x38, 0x901, b"unknown").bytes();
    let archive = archive(temporary.path(), &image, &directional)?;
    let root = temporary.path().join("out");
    let options = ExportOptions {
        diagnostic: true,
        ..options()
    };
    let ExportOutcome::Incomplete { diagnostic, .. } =
        p4kx_media::export(&archive, &root, &options)?
    else {
        return Err("unrecognized image structures must stay diagnostic".into());
    };
    let base = diagnostic.parent().ok_or("diagnostic base")?;
    let text = fs::read_to_string(base.join("animation.jsonl"))?;
    let rows = text
        .lines()
        .map(serde_json::from_str::<MediaRecord>)
        .collect::<Result<Vec<_>, _>>()?;
    validate_rows(&rows)?;
    for (family, original, reason) in [
        ("caf-image", &image, "lacks chunk"),
        (
            "directional-blend-image",
            &directional,
            "needs an IVO container",
        ),
    ] {
        check_image(base, &rows, family, original, reason)?;
    }
    Ok(())
}

fn check_image(
    base: &Path,
    rows: &[MediaRecord],
    family: &str,
    original: &[u8],
    reason: &str,
) -> TestResult {
    let row = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "animation-entry"
                && row.data.pointer("/payload/family").and_then(Value::as_str) == Some(family)
        })
        .ok_or("selected image entry")?;
    assert_eq!(
        row.status,
        Status::Partial,
        "unrecognized image cannot be complete"
    );
    assert!(
        row.issues.iter().any(|issue| issue.detail.contains(reason)),
        "specific structure diagnosis"
    );
    let artifact = row.source.raw.as_ref().ok_or("retained image Entry")?;
    assert_eq!(
        fs::read(base.join(&artifact.path))?,
        original,
        "exact original bytes"
    );
    assert!(
        rows.iter().any(|chunk| {
            chunk.subresource.namespace == "animation-chunk"
                && chunk.source.identity == row.source.identity
        }),
        "original unknown chunk must remain navigable"
    );
    Ok(())
}
