// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::atomic::AtomicBool;

use p4kx_archive::{Archive, MetadataSection, MetadataSpan, OpenOptions};

pub(super) type Result<Value = ()> = std::result::Result<Value, Box<dyn Error>>;

pub(super) struct Opened {
    pub(super) archive: Archive,
    _directory: tempfile::TempDir,
}

pub(super) fn open(bytes: &[u8]) -> Result<Opened> {
    open_with(bytes, false)
}

pub(super) fn open_with(bytes: &[u8], recover: bool) -> Result<Opened> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("synthetic.p4k");
    std::fs::write(&path, bytes)?;
    let archive = Archive::open_with(&path, OpenOptions::new().recover(recover))?;
    Ok(Opened {
        archive,
        _directory: directory,
    })
}

pub(super) fn put(bytes: &mut [u8], offset: usize, value: &[u8]) -> Result {
    let end = offset
        .checked_add(value.len())
        .ok_or("fixture offset overflow")?;
    bytes
        .get_mut(offset..end)
        .ok_or("fixture word outside authored record")?
        .copy_from_slice(value);
    Ok(())
}

pub(super) fn read<'source>(
    archive: &'source Archive,
    span: MetadataSpan<'source>,
) -> Result<Vec<u8>> {
    let mut bytes = vec![0; usize::try_from(span.range().len())?];
    let mut charge = |_| Ok(());
    let cancelled = AtomicBool::new(false);
    archive
        .metadata_view()
        .access(&mut charge, &cancelled)
        .read_into(span, 0, &mut bytes)?;
    Ok(bytes)
}

pub(super) fn section(archive: &Archive, section: MetadataSection) -> Result<Vec<u8>> {
    let span = archive
        .metadata_view()
        .span(section)?
        .ok_or("missing metadata section")?;
    read(archive, span)
}
