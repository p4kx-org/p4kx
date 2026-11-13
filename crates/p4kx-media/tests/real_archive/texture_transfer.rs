// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io::{self, Read, Write};

use p4kx_extraction::texture_preview::DdsTransfer;
use p4kx_extraction::{Archive, ArchiveWatch, DdsRawHeader, Entry, MAX_DDS_HEADER_LEN, Texture};

use super::{TestResult, privacy};

const GENERAL_SAMPLE: usize = 2048;
const DIFFUSE_SAMPLE: usize = 256;
const CRY_MARKER: [u8; 4] = *b"FYRC";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_dds_transfer_matches_original_declarations() -> Result<(), privacy::VerificationFailure>
{
    privacy::redact(verify())
}

fn verify() -> TestResult {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        p4kx_testkit::real_archive_key().ok(),
    )?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let headers: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| Texture::is_header_path(entry.path()))
        .collect();
    let diffuse: Vec<_> = headers
        .iter()
        .copied()
        .filter(|entry| {
            entry
                .path()
                .as_bytes()
                .last_chunk::<9>()
                .is_some_and(|extension| extension.eq_ignore_ascii_case(b"_diff.dds"))
        })
        .collect();
    let general = survey(&archive, &watch, &headers, GENERAL_SAMPLE)?;
    let diffuse = survey(&archive, &watch, &diffuse, DIFFUSE_SAMPLE)?;
    if general.is_empty() || !has_both_cry_transfers(&diffuse) {
        return Err("required original DDS transfer strata are absent".into());
    }
    watch.check()?;
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "DDS header sample {general:?}; diffuse suffix sample {diffuse:?}"
    )?;
    Ok(())
}

type Strata = BTreeMap<(&'static str, &'static str, bool), usize>;

fn survey(
    archive: &Archive,
    watch: &ArchiveWatch,
    entries: &[&Entry],
    maximum: usize,
) -> TestResult<Strata> {
    let count = entries.len().min(maximum);
    let mut tally = BTreeMap::new();
    let mut prefix = Vec::with_capacity(MAX_DDS_HEADER_LEN);
    for ordinal in 0..count {
        watch.poll()?;
        let position = ordinal * entries.len() / count;
        let entry = entries.get(position).ok_or("DDS sample index")?;
        prefix.clear();
        archive
            .open_entry(entry)?
            .take(MAX_DDS_HEADER_LEN as u64)
            .read_to_end(&mut prefix)?;
        let raw = DdsRawHeader::parse(&prefix)?;
        let (profile, cry) = verify_words(&raw, &prefix)?;
        *tally
            .entry((profile, cry, raw.transfer().conflict))
            .or_insert(0) += 1;
    }
    Ok(tally)
}

fn verify_words(raw: &DdsRawHeader, prefix: &[u8]) -> TestResult<(&'static str, &'static str)> {
    let base = if prefix.starts_with(b"DDS ") { 4 } else { 0 };
    let marker = bytes(prefix, base + 120)?;
    let flags = u32::from_le_bytes(bytes(prefix, base + 32)?);
    let expected_cry = if marker != CRY_MARKER {
        DdsTransfer::Unknown
    } else if flags & 0x800 == 0 {
        DdsTransfer::Linear
    } else {
        DdsTransfer::Srgb
    };
    let (profile, expected_dxgi) = if &bytes(prefix, base + 80)? == b"DX10" {
        let format = u32::from_le_bytes(bytes(prefix, base + 124)?);
        (
            "DX10",
            p4kx_extraction::DxgiFormat(format).declared_transfer(),
        )
    } else {
        ("legacy", DdsTransfer::Unknown)
    };
    let actual = raw.transfer();
    let conflict = expected_cry != DdsTransfer::Unknown
        && expected_dxgi != DdsTransfer::Unknown
        && expected_cry != expected_dxgi;
    let effective = if conflict {
        DdsTransfer::Unknown
    } else if expected_cry == DdsTransfer::Unknown {
        expected_dxgi
    } else {
        expected_cry
    };
    if (
        actual.cry_marker,
        actual.cry_image_flags,
        actual.cry,
        actual.dxgi,
        actual.effective,
        actual.conflict,
    ) != (
        u32::from_le_bytes(marker),
        flags,
        expected_cry,
        expected_dxgi,
        effective,
        conflict,
    ) {
        return Err("DDS transfer projection differs from original prefix".into());
    }
    let transfer = match expected_cry {
        DdsTransfer::Linear => "Cry linear",
        DdsTransfer::Srgb => "Cry srgb",
        _ => "Cry unknown",
    };
    Ok((profile, transfer))
}

fn bytes(prefix: &[u8], offset: usize) -> TestResult<[u8; 4]> {
    Ok(prefix
        .get(offset..offset + 4)
        .ok_or("DDS word outside prefix")?
        .try_into()?)
}

fn has_both_cry_transfers(strata: &Strata) -> bool {
    ["Cry linear", "Cry srgb"]
        .iter()
        .all(|wanted| strata.keys().any(|(_, actual, _)| actual == wanted))
}
