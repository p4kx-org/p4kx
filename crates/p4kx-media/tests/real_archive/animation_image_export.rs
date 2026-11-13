// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, Filter};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord};
use serde_json::Value;

use super::shader_accounting::require;
use super::{TestResult, privacy};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn global_animation_image_export_matches_original_path_fields()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured())
}

fn verify_configured() -> TestResult {
    verify(&p4kx_testkit::real_archive_path()?)?;
    if let Some(path) = p4kx_testkit::real_archive_b_path() {
        verify(&path)?;
    }
    Ok(())
}

fn verify(path: &Path) -> TestResult {
    let archive = Archive::open(path)?;
    let watch = ArchiveWatch::new(&archive);
    let scratch = tempfile::tempdir()?;
    let options = ExportOptions {
        kinds: vec![MediaKind::Animation],
        include: Some(Filter::parse("Data/Animations/*.img")?),
        ..Default::default()
    };
    let ExportOutcome::Published { index, .. } =
        p4kx_media::export(&archive, scratch.path(), &options)?
    else {
        return Err("animation images did not publish a complete generation".into());
    };
    let generation = scratch.path().join(&index.artifact_base);
    let caf = source(&archive, "Data/Animations/Animations.img")?;
    let directional = source(&archive, "Data/Animations/DirectionalBlends.img")?;
    let stream = index
        .kinds
        .iter()
        .find(|kind| kind.provider.kind == MediaKind::Animation)
        .ok_or("animation stream")?;
    let mut count = Counts::default();
    for line in BufReader::new(File::open(generation.join(&stream.artifact.path))?).lines() {
        let row: MediaRecord = serde_json::from_str(&line?)?;
        match row.subresource.namespace.as_str() {
            "animation-image-record" => {
                compare_caf(&row, &caf, count.caf)?;
                count.caf += 1;
            }
            "animation-image-directional-path" => {
                let start = compare_path(&row, &directional)?;
                require(
                    start >= count.last_directional_end,
                    "ordered AIM path spans",
                )?;
                count.last_directional_end = start + 256;
                count.directional += 1;
            }
            _ => {}
        }
    }
    require(
        count.caf == declared_count(&caf, 0x3702_75B7)?,
        "CAF image reconciles",
    )?;
    require(
        count.directional == declared_count(&directional, 0x6B8D_F259)?,
        "directional image reconciles",
    )?;
    watch.check()?;
    scratch.close()?;
    Ok(())
}

#[derive(Default)]
struct Counts {
    caf: usize,
    directional: usize,
    last_directional_end: usize,
}

fn source(archive: &Archive, path: &str) -> TestResult<Vec<u8>> {
    let entry = p4kx_extraction::find_entry(archive, path).ok_or("image Entry")?;
    let mut original = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut original)?;
    Ok(original)
}

fn compare_path(row: &MediaRecord, original: &[u8]) -> TestResult<usize> {
    let start = number(row, "/payload/path_field/start")?;
    let end = number(row, "/payload/path_field/end_exclusive")?;
    require(end.checked_sub(start) == Some(256), "exact path width")?;
    let field = original.get(start..end).ok_or("original path range")?;
    let nul = field
        .iter()
        .position(|byte| *byte == 0)
        .ok_or("path terminator")?;
    let path = std::str::from_utf8(field.get(..nul).ok_or("original path")?)?;
    require(
        row.data
            .pointer("/payload/path/authored")
            .and_then(Value::as_str)
            == Some(path),
        "exact path text",
    )?;
    Ok(start)
}

fn compare_caf(row: &MediaRecord, original: &[u8], ordinal: usize) -> TestResult {
    let start = compare_path(row, original)?;
    require(
        row.subresource.indices.as_slice() == [0, ordinal as u64],
        "original clip ordinal",
    )?;
    let caf = original.get(start + 256..start + 260).ok_or("CAF word")?;
    let dba = original.get(start + 260..start + 264).ok_or("DBA word")?;
    let expected_dba = u32::from_le_bytes(dba.try_into()?);
    require(
        number(row, "/payload/caf_path_crc32")? == u32::from_le_bytes(caf.try_into()?) as usize,
        "original CAF hash",
    )?;
    let exported = row
        .data
        .pointer("/payload/dba_path_crc32")
        .and_then(Value::as_u64);
    require(
        exported == (expected_dba != 0).then_some(u64::from(expected_dba)),
        "original DBA hash",
    )
}

fn number(row: &MediaRecord, path: &str) -> TestResult<usize> {
    let number = row
        .data
        .pointer(path)
        .and_then(Value::as_u64)
        .ok_or("numeric source coordinate")?;
    Ok(usize::try_from(number)?)
}

fn declared_count(original: &[u8], kind: u32) -> TestResult<usize> {
    let file = p4kx_extraction::chunks::ChunkFile::parse(original)?;
    let (index, _) = file
        .chunks()
        .iter()
        .enumerate()
        .find(|(_, chunk)| chunk.chunk_type().id() == kind)
        .ok_or("original image chunk")?;
    let prefix = file
        .chunk_data(index)
        .and_then(|bytes| bytes.first_chunk::<4>())
        .ok_or("image count")?;
    Ok(u32::from_le_bytes(*prefix) as usize)
}
