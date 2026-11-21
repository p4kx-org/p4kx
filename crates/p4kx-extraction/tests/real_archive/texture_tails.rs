// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Every alpha tail of the real Archive surveyed by layout: which `.dds.a` holds a header, which
//! holds raw pixels, how many numbered parts `.dds.Na` there are, and whether any raw tail fits
//! the mips of its colour texture exactly.

use std::collections::{BTreeMap, HashSet};
use std::io::Read;

use p4kx_extraction::{Archive, DdsHeader, Entry, Texture};
use p4kx_testkit::real_archive_path;
use p4kx_texture::MAX_HEADER_LEN;
use rayon::prelude::*;

use crate::{TestResult, report};

const MAGIC: &[u8; 4] = b"DDS ";
const HEADER_SIZE: [u8; 4] = 124_u32.to_le_bytes();

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Layout {
    HeaderWithMagicAndParts,
    HeaderWithMagicOnly,
    HeaderWithoutMagicAndParts,
    HeaderWithoutMagicOnly,
    RawFitsR8,
    RawFitsBc4,
    RawFitsNeither,
    RawWithoutColour,
}

fn lower(entry: &Entry) -> String {
    entry.path().to_ascii_lowercase().replace('\\', "/")
}

fn mip_chain(width: u32, height: u32, mips: u32, bytes: u64, block: u32) -> u64 {
    (0..mips)
        .map(|level| {
            let side = |full: u32| u64::from((full >> level).max(1).div_ceil(block));
            side(width) * side(height) * bytes
        })
        .sum()
}

fn classify(
    archive: &Archive,
    tail: &Entry,
    part_names: &HashSet<String>,
) -> Result<Layout, Box<dyn std::error::Error + Send + Sync>> {
    let mut start = Vec::with_capacity(MAX_HEADER_LEN);
    archive
        .open_entry(tail)?
        .take(MAX_HEADER_LEN as u64)
        .read_to_end(&mut start)?;
    let name = lower(tail);
    let parts = part_names.contains(&format!("{}1a", name.trim_end_matches('a')));
    let magic = start.starts_with(MAGIC);
    if (magic || start.starts_with(&HEADER_SIZE)) && DdsHeader::parse(&start).is_ok() {
        return Ok(match (magic, parts) {
            (true, true) => Layout::HeaderWithMagicAndParts,
            (true, false) => Layout::HeaderWithMagicOnly,
            (false, true) => Layout::HeaderWithoutMagicAndParts,
            (false, false) => Layout::HeaderWithoutMagicOnly,
        });
    }
    let colour = tail.path().get(..tail.path().len() - 2).unwrap_or_default();
    let Ok(texture) = Texture::from_archive(archive, colour) else {
        return Ok(Layout::RawWithoutColour);
    };
    let header = texture.header();
    let surfaces = header.surfaces();
    let size = tail.uncompressed_size();
    let fits = |bytes: u64, block: u32| {
        mip_chain(
            header.width(),
            header.height(),
            header.mip_count(),
            bytes,
            block,
        )
        .checked_mul(surfaces)
            == Some(size)
    };
    Ok(if fits(1, 1) {
        Layout::RawFitsR8
    } else if fits(8, 4) {
        Layout::RawFitsBc4
    } else {
        Layout::RawFitsNeither
    })
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn alpha_tails_are_surveyed_by_layout() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let names: HashSet<String> = archive.entries().iter().map(lower).collect();
    let tails: Vec<&Entry> = archive
        .entries()
        .par_iter()
        .filter(|entry| lower(entry).ends_with(".dds.a"))
        .collect();
    let classified: Vec<Result<Layout, String>> = tails
        .par_iter()
        .map(|tail| classify(&archive, tail, &names).map_err(|error| error.to_string()))
        .collect();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut failures = Vec::new();
    for layout in classified {
        match layout {
            Ok(layout) => *counts.entry(format!("{layout:?}")).or_default() += 1,
            Err(error) => failures.push(error),
        }
    }
    let orphans = names
        .iter()
        .filter(|name| {
            let Some((base, part)) = name.rsplit_once('.') else {
                return false;
            };
            part.len() > 1
                && part.ends_with('a')
                && part[..part.len() - 1]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit())
                && base.rsplit('.').next() == Some("dds")
                && !names.contains(&format!("{base}.a"))
        })
        .count();
    report(&format!(
        "{} alpha tails by layout: {counts:#?}; numbered tail parts without a tail header: \
         {orphans}",
        tails.len()
    ));
    assert!(
        failures.is_empty(),
        "tails that could not be read: {failures:?}"
    );
    let raw = counts
        .iter()
        .filter(|(layout, _)| layout.starts_with("Raw"))
        .map(|(_, count)| count)
        .sum::<usize>();
    assert_eq!(raw, 0, "a headerless alpha tail appeared: {counts:?}");
    assert_eq!(orphans, 0, "numbered tail parts without a tail header");
    Ok(())
}
