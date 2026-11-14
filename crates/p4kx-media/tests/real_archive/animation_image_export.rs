// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, Exclusion, Filter};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord, Status};
use serde_json::Value;

use super::shader_accounting::require;
use super::{TestResult, manifests, privacy};

#[path = "animation_directional_record_oracle.rs"]
mod directional_oracle;

const CAF_IMAGE: &str = "Data/Animations/Animations.img";
const DIRECTIONAL_IMAGE: &str = "Data/Animations/DirectionalBlends.img";
const CAF_CHUNK: u32 = 0x3702_75B7;
const DIRECTIONAL_CHUNK: u32 = 0x6B8D_F259;
const CAF_HEADER_LENGTH: usize = 8;
const CAF_RECORD_LENGTH: usize = 308;
const CAF_TRAILER_LENGTH: usize = 8;
const PATH_LENGTH: usize = 256;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn global_animation_image_export_matches_original_path_fields()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured(Scope::Both))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn directional_image_pose_export_matches_original_ranges()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured(Scope::Directional))
}

#[derive(Clone, Copy)]
enum Scope {
    Both,
    Directional,
}

impl Scope {
    const fn filter(self) -> &'static str {
        match self {
            Self::Both => "Data/Animations/*.img",
            Self::Directional => DIRECTIONAL_IMAGE,
        }
    }
}

fn verify_configured(scope: Scope) -> TestResult {
    verify(&p4kx_testkit::real_archive_path()?, scope)?;
    if let Some(path) = p4kx_testkit::real_archive_b_path() {
        verify(&path, scope)?;
    }
    Ok(())
}

fn verify(path: &Path, scope: Scope) -> TestResult {
    verify_archive(&Archive::open(path)?, scope)
}

fn verify_archive(archive: &Archive, scope: Scope) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    let scratch = tempfile::tempdir()?;
    let options = ExportOptions {
        kinds: vec![MediaKind::Animation],
        include: Some(Filter::parse(scope.filter())?),
        exclude: Exclusion::parse(["*.pak", "*.socpak"])?,
        ..Default::default()
    };
    let ExportOutcome::Published { index, .. } =
        p4kx_media::export(archive, scratch.path(), &options)?
    else {
        return Err("animation images did not publish a complete generation".into());
    };
    let generation = scratch.path().join(&index.artifact_base);
    let caf = matches!(scope, Scope::Both)
        .then(|| ImageSource::read(archive, CAF_IMAGE, CAF_CHUNK, 0x902))
        .transpose()?;
    let directional = ImageSource::read(archive, DIRECTIONAL_IMAGE, DIRECTIONAL_CHUNK, 0x901)?;
    let framed = directional_oracle::Records::read(&directional)?;
    if matches!(scope, Scope::Directional) {
        framed.require_poses()?;
    }
    let stream = index
        .kinds
        .iter()
        .find(|kind| kind.provider.kind == MediaKind::Animation)
        .ok_or("animation stream")?;
    let rows = BufReader::new(File::open(generation.join(&stream.artifact.path))?);
    verify_rows(&generation, rows, caf.as_ref(), &directional, &framed)?;
    watch.check()?;
    scratch.close()?;
    Ok(())
}

struct ImageSource {
    bytes: Vec<u8>,
    chunk: PhysicalChunk,
    count: usize,
}

impl ImageSource {
    fn read(archive: &Archive, path: &str, kind: u32, version: u32) -> TestResult<Self> {
        let bytes = source(archive, path)?;
        let chunk = physical_chunk(&bytes, kind, version)?;
        let count = declared_count(&bytes, chunk.start)?;
        Ok(Self {
            bytes,
            chunk,
            count,
        })
    }

    fn verify_raw(&self, generation: &Path, row: &MediaRecord) -> TestResult<Vec<u8>> {
        let raw = row.source.raw.as_ref().ok_or("original image artifact")?;
        let retained = manifests::check_payload(
            generation,
            &raw.path,
            raw.size,
            &raw.digest,
            self.bytes.len() as u64,
        )?;
        require(
            retained == self.bytes,
            "retained image equals its verified original",
        )?;
        Ok(retained)
    }
}

#[derive(Default)]
struct Coverage {
    records: usize,
    indices: usize,
    raw_entries: usize,
}

impl Coverage {
    const fn verified(&self, source: &ImageSource) -> bool {
        self.records == source.count && self.indices == 1 && self.raw_entries == 1
    }
}

fn verify_rows(
    generation: &Path,
    rows: impl BufRead,
    caf: Option<&ImageSource>,
    directional: &ImageSource,
    framed: &directional_oracle::Records<'_>,
) -> TestResult {
    let mut caf_observed = Coverage::default();
    let mut directional_observed = Coverage::default();
    let mut framed_observed = framed.replay();
    for line in rows.lines() {
        let row: MediaRecord = serde_json::from_str(&line?)?;
        match row.subresource.namespace.as_str() {
            "animation-image-record" => {
                compare_caf(&row, caf.ok_or("unexpected CAF row")?, caf_observed.records)?;
                caf_observed.records += 1;
            }
            "animation-image-directional-path" => {
                framed_observed.path(&row)?;
                directional_observed.records += 1;
            }
            "animation-image-directional-record" => framed_observed.record(&row)?,
            "animation-image-directional-pose-index" => framed_observed.pose_index(&row)?,
            "animation-image-directional-pose-block" => framed_observed.pose_block(&row)?,
            "animation-entry" => {
                let (source, count) = source_of(
                    &row,
                    caf,
                    directional,
                    &mut caf_observed,
                    &mut directional_observed,
                )?;
                let raw = source.verify_raw(generation, &row)?;
                if std::ptr::eq(source, directional) {
                    framed_observed.artifact(raw)?;
                }
                count.raw_entries += 1;
            }
            "animation-image-index" => {
                let status = row
                    .data
                    .pointer("/payload/layout/status")
                    .and_then(Value::as_str);
                match status {
                    Some("caf-records") => {
                        verify_caf_index(&row, caf.ok_or("unexpected CAF index")?)?;
                        caf_observed.indices += 1;
                    }
                    Some("directional-records") => {
                        verify_index(&row, &directional.chunk)?;
                        framed_observed.index(&row)?;
                        directional_observed.indices += 1;
                    }
                    _ => return Err("unexpected projected image layout".into()),
                }
            }
            _ => {}
        }
    }
    if let Some(caf) = caf {
        require(caf_observed.verified(caf), "CAF image reconciles")?;
    }
    framed_observed.finish()?;
    require(
        directional_observed.verified(directional),
        "directional image reconciles",
    )
}

fn source_of<'a, 'b>(
    row: &MediaRecord,
    caf: Option<&'a ImageSource>,
    directional: &'a ImageSource,
    caf_observed: &'b mut Coverage,
    directional_observed: &'b mut Coverage,
) -> TestResult<(&'a ImageSource, &'b mut Coverage)> {
    match row.data.pointer("/payload/family").and_then(Value::as_str) {
        Some("caf-image") => Ok((caf.ok_or("unexpected CAF Entry")?, caf_observed)),
        Some("directional-blend-image") => Ok((directional, directional_observed)),
        _ => Err("global image verification selected an unrelated animation".into()),
    }
}

fn verify_caf_index(row: &MediaRecord, source: &ImageSource) -> TestResult {
    let chunk = &source.chunk;
    let records_start = chunk.start + CAF_HEADER_LENGTH;
    let trailer_start = records_start
        .checked_add(
            source
                .count
                .checked_mul(CAF_RECORD_LENGTH)
                .ok_or("CAF table size")?,
        )
        .ok_or("CAF trailer position")?;
    verify_index(row, chunk)?;
    require(
        trailer_start.checked_add(CAF_TRAILER_LENGTH) == Some(chunk.end),
        "original CAF count frames the exact trailer",
    )?;
    verify_range(row, "/payload/layout/header", chunk.start, records_start)?;
    verify_range(row, "/payload/layout/trailer", trailer_start, chunk.end)?;
    require(
        number(row, "/payload/layout/record_count")? == source.count,
        "exact CAF count declaration",
    )
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
    require(
        end.checked_sub(start) == Some(PATH_LENGTH),
        "exact path width",
    )?;
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

fn compare_caf(row: &MediaRecord, source: &ImageSource, ordinal: usize) -> TestResult {
    let original = &source.bytes;
    let start = compare_path(row, original)?;
    require(
        ordinal < source.count
            && row.subresource.indices.as_slice() == [source.chunk.index as u64, ordinal as u64],
        "original clip ordinal",
    )?;
    let expected_start = source.chunk.start + CAF_HEADER_LENGTH + ordinal * CAF_RECORD_LENGTH;
    verify_range(
        row,
        "/payload/range",
        expected_start,
        expected_start + CAF_RECORD_LENGTH,
    )?;
    require(start == expected_start, "exact CAF path field position")?;
    let caf = original
        .get(start + PATH_LENGTH..start + 260)
        .ok_or("CAF word")?;
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

struct PhysicalChunk {
    index: usize,
    start: usize,
    end: usize,
}

fn physical_chunk(original: &[u8], kind: u32, version: u32) -> TestResult<PhysicalChunk> {
    let file = p4kx_extraction::chunks::ChunkFile::parse(original)?;
    let mut matches = file
        .chunks()
        .iter()
        .enumerate()
        .filter(|(_, chunk)| chunk.chunk_type().id() == kind);
    let (index, chunk) = matches.next().ok_or("original image chunk")?;
    require(
        matches.next().is_none() && chunk.version() == version,
        "a single versioned image chunk",
    )?;
    Ok(PhysicalChunk {
        index,
        start: usize::try_from(chunk.offset())?,
        end: usize::try_from(chunk.offset() + chunk.size())?,
    })
}

fn declared_count(original: &[u8], offset: usize) -> TestResult<usize> {
    let prefix = original.get(offset..offset + 4).ok_or("image count")?;
    Ok(usize::try_from(u32::from_le_bytes(prefix.try_into()?))?)
}

fn verify_index(row: &MediaRecord, chunk: &PhysicalChunk) -> TestResult {
    require(
        row.status == Status::Complete
            && row.subresource.indices.as_slice() == [chunk.index as u64],
        "index belongs to its original chunk",
    )?;
    verify_range(row, "/payload/range", chunk.start, chunk.end)
}

fn verify_range(row: &MediaRecord, path: &str, start: usize, end: usize) -> TestResult {
    let range = row.data.pointer(path).ok_or("image source range")?;
    require(
        range.pointer("/basis/type").and_then(Value::as_str) == Some("entry")
            && range.get("start").and_then(Value::as_u64) == Some(start as u64)
            && range.get("end_exclusive").and_then(Value::as_u64) == Some(end as u64),
        "exact original entry-relative range",
    )
}
