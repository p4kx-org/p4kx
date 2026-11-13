// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;
use std::path::Path;

use p4kx_extraction::chunks::ChunkFile;
use p4kx_extraction::model::{CafImage, CafImagePath};
use p4kx_extraction::{Archive, ArchiveWatch};

use super::shader_accounting::require;
use super::{TestResult, privacy};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn global_animation_images_match_verified_physical_sources()
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
    verify_caf(&read(&archive, "Data/Animations/Animations.img")?)?;
    verify_directional(&read(&archive, "Data/Animations/DirectionalBlends.img")?)?;
    watch.check()?;
    Ok(())
}

fn read(archive: &Archive, path: &str) -> TestResult<ChunkFile<Vec<u8>>> {
    let entry = p4kx_extraction::find_entry(archive, path).ok_or("original image Entry")?;
    let mut bytes = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut bytes)?;
    Ok(ChunkFile::parse(bytes)?)
}

fn verify_caf(file: &ChunkFile<Vec<u8>>) -> TestResult {
    let (offset, bytes) = chunk(file, 0x3702_75B7, 0x902)?;
    let declared = u32::from_le_bytes(bytes.get(..4).ok_or("counted prefix")?.try_into()?) as usize;
    let body = bytes
        .get(8..bytes.len().checked_sub(8).ok_or("CAF trailer")?)
        .ok_or("CAF body")?;
    let (records, remainder) = body.as_chunks::<308>();
    require(
        records.len() == declared && remainder.is_empty(),
        "exact counted CAF extent",
    )?;
    let actual = CafImage::read(bytes, offset)?;
    require(actual.record_count() == declared, "CAF record count")?;
    require(
        actual.header_range() == (offset..offset + 8),
        "CAF header range",
    )?;
    for (ordinal, (raw, record)) in records.iter().zip(actual.records()).enumerate() {
        let field = raw.get(..256).ok_or("CAF path field")?;
        let nul = field
            .iter()
            .position(|byte| *byte == 0)
            .ok_or("CAF path terminator")?;
        let text = std::str::from_utf8(field.get(..nul).ok_or("CAF original path")?)?;
        let start = offset + 8 + ordinal as u64 * 308;
        require(
            record.range() == (start..start + 308),
            "original CAF record range",
        )?;
        require(
            record.path_field_range() == (start..start + 256),
            "original path range",
        )?;
        require(
            record.path() == CafImagePath::Text(text),
            "original path spelling",
        )?;
    }
    let end = offset + bytes.len() as u64;
    require(
        actual.trailer_range() == (end - 8..end),
        "CAF trailer range",
    )
}

fn verify_directional(file: &ChunkFile<Vec<u8>>) -> TestResult {
    let (_, bytes) = chunk(file, 0x6B8D_F259, 0x901)?;
    let prefix = bytes.get(..4).ok_or("directional count word")?;
    let declared = u32::from_le_bytes(prefix.try_into()?) as usize;
    let mut observed = 0;
    for (start, prefix) in bytes.windows(b"animations/".len()).enumerate() {
        if start % 4 != 0 || !prefix.eq_ignore_ascii_case(b"animations/") {
            continue;
        }
        let Some(field) = bytes.get(start..start + 256) else {
            continue;
        };
        let Some(nul) = field.iter().position(|byte| *byte == 0) else {
            continue;
        };
        if field.get(..nul).is_some_and(|path| path.ends_with(b".aim"))
            && field
                .get(nul..)
                .is_some_and(|tail| tail.iter().all(|byte| *byte == 0))
        {
            observed += 1;
        }
    }
    require(
        observed != 0 && observed == declared,
        "directional opaque path accounting",
    )
}

fn chunk(file: &ChunkFile<Vec<u8>>, kind: u32, version: u32) -> TestResult<(u64, &[u8])> {
    let mut matches = file
        .chunks()
        .iter()
        .enumerate()
        .filter(|(_, chunk)| chunk.chunk_type().id() == kind);
    let (index, chunk) = matches.next().ok_or("recognized image chunk")?;
    require(
        matches.next().is_none() && chunk.version() == version,
        "one versioned image chunk",
    )?;
    Ok((
        chunk.offset(),
        file.chunk_data(index).ok_or("retained chunk")?,
    ))
}
