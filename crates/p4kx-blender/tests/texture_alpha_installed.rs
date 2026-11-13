// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded installed DDS alpha declarations and decoded coverage.

use std::io::{self, Read as _, Write as _};

use p4kx_extraction::texture_codec::{
    DdsAlphaAssociation, DdsAlphaCoverage, DxgiFormat, TextureFormat,
};
use p4kx_extraction::{
    Archive, ArchiveWatch, DdsHeader, DdsRawHeader, Entry, MAX_DDS_HEADER_LEN, Texture,
};

type Check<T = ()> = Result<T, Box<dyn std::error::Error>>;

const SAMPLE_HEADERS: usize = 512;
const MAX_MIP_PIXELS: u64 = 256 * 256;
const PER_FORMAT: usize = 8;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_bc_alpha_coverage_matches_decoded_selected_mips() -> Check {
    verify().map_err(|_| "installed DDS alpha witness failed".into())
}

fn verify() -> Check {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        p4kx_testkit::real_archive_key().ok(),
    )?;
    let watch = ArchiveWatch::new(&archive);
    let candidates: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .path()
                .as_bytes()
                .last_chunk::<4>()
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(b".dds"))
        })
        .collect();
    let samples = candidates.len().min(SAMPLE_HEADERS);
    let mut decoded = [(0, 0); 3];

    let mut declarations = [false; 7];
    let mut readable = 0;
    watch.check()?;
    for ordinal in 0..samples {
        watch.poll()?;
        let entry = *candidates
            .get(ordinal * candidates.len() / samples)
            .ok_or("sample position")?;
        let Ok((raw, header)) = header(&archive, entry) else {
            continue;
        };
        readable += 1;
        let association = raw.alpha_association();
        *declarations
            .get_mut(declaration(association))
            .ok_or("declaration stratum")? = true;
        let Some(group) = block_group(header.format()) else {
            continue;
        };
        let counts = decoded.get_mut(group).ok_or("compressed stratum")?;
        if counts.0 < PER_FORMAT
            && let Some(coverage) = coverage(&archive, entry, &header, association)?
        {
            counts.0 += 1;
            counts.1 += usize::from(coverage.has_non_full_alpha());
        }
    }
    watch.check()?;
    if !decoded.into_iter().any(|(count, _)| count > 0) {
        return Err("no selected compressed surface was decoded".into());
    }
    writeln!(
        io::stdout().lock(),
        "DDS header declarations present (unknown, straight, premultiplied, opaque, custom, reserved, other): {declarations:?}; readable headers {readable}; BC1/BC2/BC3 (decoded, non-full alpha) {decoded:?}"
    )?;
    Ok(())
}

fn coverage(
    archive: &Archive,
    entry: &Entry,
    header: &DdsHeader,
    association: DdsAlphaAssociation,
) -> Check<Option<DdsAlphaCoverage>> {
    let level = header.mip_count().saturating_sub(1);
    let side = |value: u32| u64::from(value.checked_shr(level).unwrap_or_default().max(1));
    if header.images(level) != 1 || side(header.width()) * side(header.height()) > MAX_MIP_PIXELS {
        return Ok(None);
    }
    let Ok(texture) = Texture::from_entry(archive, entry) else {
        return Ok(None);
    };
    if texture.source_metadata().alpha_association != Some(association) {
        return Err("selected header declaration differs".into());
    }
    let Ok(mip) = texture.read_mip(level) else {
        return Ok(None);
    };
    let coverage = mip.alpha_coverage(0).ok_or("selected BC alpha coverage")?;
    let pixels = mip.decode()?;
    let (mut zero, mut partial, mut full) = (0, 0, 0);
    for &[_, _, _, opacity] in pixels.pixels().as_chunks::<4>().0 {
        match opacity {
            0 => zero += 1,
            u8::MAX => full += 1,
            _ => partial += 1,
        }
    }
    if (coverage.zero, coverage.partial, coverage.full) != (zero, partial, full) {
        return Err("selected BC blocks differ from decoded texels".into());
    }
    Ok(Some(coverage))
}

fn header(archive: &Archive, entry: &Entry) -> Check<(DdsRawHeader, DdsHeader)> {
    let mut bytes = Vec::with_capacity(MAX_DDS_HEADER_LEN);
    archive
        .open_entry(entry)?
        .take(MAX_DDS_HEADER_LEN as u64)
        .read_to_end(&mut bytes)?;
    let raw = DdsRawHeader::parse(&bytes)?;
    Ok((raw, DdsHeader::from_raw(&raw)?))
}

const fn block_group(format: TextureFormat) -> Option<usize> {
    match format {
        TextureFormat::Dxgi(DxgiFormat(70..=72)) => Some(0),
        TextureFormat::Dxgi(DxgiFormat(73..=75)) => Some(1),
        TextureFormat::Dxgi(DxgiFormat(76..=78)) => Some(2),
        _ => None,
    }
}

const fn declaration(association: DdsAlphaAssociation) -> usize {
    match association {
        DdsAlphaAssociation::Unknown => 0,
        DdsAlphaAssociation::Straight => 1,
        DdsAlphaAssociation::Premultiplied => 2,
        DdsAlphaAssociation::Opaque => 3,
        DdsAlphaAssociation::Custom => 4,
        DdsAlphaAssociation::Reserved => 5,
        _ => 6,
    }
}
