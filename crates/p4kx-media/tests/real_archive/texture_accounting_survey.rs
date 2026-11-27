// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io::{self, Read};

use p4kx_extraction::texture_codec::{DdsPngEncoding, DdsTransfer};
use p4kx_extraction::{
    Archive, ArchiveWatch, DdsHeader, DdsRawHeader, DdsSourceInfo, Entry, MAX_DDS_HEADER_LEN,
    PathIndex, Texture,
};

use super::super::TestResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Class {
    Plane,
    Split,
    Alpha,
    Array,
    Cube,
    CubeArray,
    Volume,
    HighRange,
    Integer,
    LinearPng,
    SrgbPng,
}

pub(super) struct Survey<'a> {
    pub(super) samples: BTreeMap<Class, &'a Entry>,
}

#[derive(Default)]
struct Dispositions {
    unreadable: u64,
    invalid: u64,
    valid: u64,
}
pub(super) fn read<'a>(
    archive: &'a Archive,
    paths: &PathIndex<'a>,
    watch: &ArchiveWatch,
) -> TestResult<Survey<'a>> {
    let mut samples = BTreeMap::new();
    let mut total = 0_u64;
    let mut dispositions = Dispositions::default();
    let mut prefix = Vec::with_capacity(MAX_DDS_HEADER_LEN);
    for entry in archive
        .entries()
        .iter()
        .filter(|entry| Texture::is_header_path(entry.path()))
    {
        watch.poll()?;
        total += 1;
        prefix.clear();
        let Ok(reader) = archive.open_entry(entry) else {
            dispositions.unreadable += 1;
            continue;
        };
        if reader
            .take(MAX_DDS_HEADER_LEN as u64)
            .read_to_end(&mut prefix)
            .is_err()
        {
            dispositions.unreadable += 1;
            continue;
        }
        let Ok(raw) = DdsRawHeader::parse(&prefix) else {
            dispositions.invalid += 1;
            continue;
        };
        let Ok((metadata, header)) =
            DdsSourceInfo::from_raw_with_header(&raw, entry.uncompressed_size())
        else {
            dispositions.invalid += 1;
            continue;
        };
        if metadata.raw.header_length != raw.header_len()
            || metadata.header_source_bytes.get() != entry.uncompressed_size()
        {
            return Err("authored DDS prefix or stored extent differs".into());
        }
        dispositions.valid += 1;
        if let Ok(parts) = header.part_count(entry.uncompressed_size()) {
            let png = DdsPngEncoding::of(header.format(), raw.transfer().effective);
            admit(
                archive,
                paths,
                entry,
                &header,
                parts,
                png.transfer,
                &mut samples,
            )?;
        }
    }
    if dispositions.valid == 0
        || total != dispositions.valid + dispositions.invalid + dispositions.unreadable
    {
        return Err("physical DDS headers were not completely inventoried".into());
    }
    Ok(Survey { samples })
}

fn admit<'a>(
    archive: &Archive,
    paths: &PathIndex<'_>,
    entry: &'a Entry,
    header: &DdsHeader,
    parts: u32,
    png_transfer: DdsTransfer,
    samples: &mut BTreeMap<Class, &'a Entry>,
) -> TestResult {
    if !header.format().is_decodable()
        || !entry.path().to_ascii_lowercase().ends_with(".dds")
        || entry.path().contains(['*', '?'])
        || archive.raw_path(entry.index()).is_some()
        || entry.uncompressed_size() > (2 << 20)
        || header.mip_size(0).is_none_or(|bytes| bytes > 2 << 20)
    {
        return Ok(());
    }
    if paths
        .matching_entries_with(entry.path(), |_| Ok::<_, io::Error>(()))?
        .count()
        != 1
    {
        return Ok(());
    }
    let alpha = paths.find(&format!("{}.a", entry.path())).is_some();
    for (present, class) in [
        (header.surfaces() == 1 && header.depth() == 1, Class::Plane),
        (parts > 0, Class::Split),
        (alpha, Class::Alpha),
        (
            header.array_size() > 1 && !header.is_cubemap(),
            Class::Array,
        ),
        (header.is_cubemap() && header.array_size() == 1, Class::Cube),
        (
            header.is_cubemap() && header.array_size() > 1,
            Class::CubeArray,
        ),
        (header.depth() > 1, Class::Volume),
        (header.format().is_high_range(), Class::HighRange),
        (
            !header.format().is_high_range() && png_transfer == DdsTransfer::Linear,
            Class::LinearPng,
        ),
        (
            !header.format().is_high_range() && png_transfer == DdsTransfer::Srgb,
            Class::SrgbPng,
        ),
        (
            header.format().to_string().contains("_UINT"),
            Class::Integer,
        ),
    ] {
        if present {
            samples.entry(class).or_insert(entry);
        }
    }
    Ok(())
}
