// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufReader, Read as _, Write as _};
use std::path::Path;

use p4kx_extraction::texture_codec::DdsTransfer;
use p4kx_extraction::{
    Archive, ArchiveWatch, CubemapLayout, DdsHeader, DdsRawHeader, Entry, MAX_DDS_HEADER_LEN,
    Texture,
};

use super::TestResult;

const DIFFUSE_SAMPLE: usize = 256;
const MAX_WITNESS_DIMENSION: u32 = 512;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_png_preserves_original_linear_and_srgb_samples() -> TestResult {
    let verified = verify().map_err(|_| "installed DDS transfer artifact verification failed")?;
    writeln!(
        io::stdout().lock(),
        "Verified standalone PNG output: {verified} original transfer and DDS profile groups"
    )?;
    Ok(())
}

fn verify() -> Result<usize, Box<dyn std::error::Error>> {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        p4kx_testkit::real_archive_key().ok(),
    )?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let diffuse: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .path()
                .as_bytes()
                .last_chunk::<9>()
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(b"_diff.dds"))
        })
        .collect();
    let count = diffuse.len().min(DIFFUSE_SAMPLE);
    let dir = tempfile::tempdir()?;
    let mut verified = [[false; 2]; 2];
    for ordinal in 0..count {
        watch.poll()?;
        let entry = diffuse
            .get(ordinal * diffuse.len() / count)
            .ok_or("DDS sample index")?;
        let (raw, transfer, profile) = read_declaration(&archive, entry)?;
        let Some(transfer) = transfer else { continue };
        let row = usize::from(transfer == DdsTransfer::Srgb);
        let slot = verified
            .get_mut(row)
            .and_then(|profiles| profiles.get_mut(profile))
            .ok_or("DDS transfer profile index")?;
        if *slot {
            continue;
        }
        let header = DdsHeader::from_raw(&raw)?;
        let Some(level) = small_colour_mip(&header) else {
            continue;
        };
        let path = dir.path().join(format!("{row}-{profile}.png"));
        check_artifact(&archive, entry, level, transfer, &path)?;
        *slot = true;
        if verified.iter().flatten().all(|&done| done) {
            break;
        }
    }
    watch.check()?;
    if !verified.iter().flatten().all(|&done| done) {
        return Err("original DDS transfer and profile groups incomplete".into());
    }
    Ok(verified.iter().flatten().count())
}

fn read_declaration(
    archive: &Archive,
    entry: &Entry,
) -> Result<(DdsRawHeader, Option<DdsTransfer>, usize), Box<dyn std::error::Error>> {
    let mut prefix = Vec::with_capacity(MAX_DDS_HEADER_LEN);
    archive
        .open_entry(entry)?
        .take(MAX_DDS_HEADER_LEN as u64)
        .read_to_end(&mut prefix)?;
    let raw = DdsRawHeader::parse(&prefix)?;
    let profile = usize::from(raw.dx10().is_some());
    let declaration = raw.transfer();
    let transfer = match declaration.effective {
        DdsTransfer::Linear | DdsTransfer::Srgb => Some(declaration.effective),
        _ => None,
    };
    Ok((raw, transfer, profile))
}

fn small_colour_mip(header: &DdsHeader) -> Option<u32> {
    let format = header.format();
    if !format.is_decodable() || format.is_high_range() {
        return None;
    }
    (0..header.mip_count()).find(|&level| {
        header.images(level) == 1
            && header
                .width()
                .max(header.height())
                .checked_shr(level)
                .unwrap_or(0)
                <= MAX_WITNESS_DIMENSION
    })
}

fn check_artifact(
    archive: &Archive,
    entry: &Entry,
    level: u32,
    transfer: DdsTransfer,
    path: &Path,
) -> TestResult {
    let texture = Texture::from_entry(archive, entry)?;
    let expected = texture.decode_mip(level)?;
    let (report, origins) = texture.png_to_file_with_origins(path, level, CubemapLayout::Faces)?;
    if report.encoding.transfer != transfer || origins.receipts.len() != 1 {
        return Err("original and encoded transfer or receipt count differs".into());
    }
    let mut decoded = png::Decoder::new(BufReader::new(File::open(path)?)).read_info()?;
    let info = decoded.info();
    if info.srgb.is_some() != (transfer == DdsTransfer::Srgb)
        || info.gama_chunk.map(png::ScaledFloat::into_value)
            != (transfer == DdsTransfer::Linear).then_some(1.0)
    {
        return Err("PNG transfer metadata differs".into());
    }
    let mut pixels = vec![0; decoded.output_buffer_size().ok_or("PNG output size")?];
    let frame = decoded.next_frame(&mut pixels)?;
    if (frame.width, frame.height, pixels.as_slice())
        != (expected.width(), expected.height(), expected.pixels())
    {
        return Err("PNG sample values differ".into());
    }
    Ok(())
}
