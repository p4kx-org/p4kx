// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::path::PathBuf;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};
use tempfile::TempDir;

use super::{ReadResult, Unread, XmlPreview, XmlStatus, read};
use crate::EntryIndex;

type TestResult = Result<(), Box<dyn Error>>;

fn opened() -> Result<(TempDir, PathBuf, EntryIndex), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let ship =
        TestElement::new("ship").to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\ship.xml", ship))
        .write_to(&path)?;
    let index = EntryIndex::build(Archive::open_with_key(
        &path,
        Some(p4kx_testkit::test_key()),
    )?);
    Ok((folder, path, index))
}

fn emptied(path: &PathBuf) -> io::Result<()> {
    OpenOptions::new().write(true).open(path)?.set_len(0)
}

struct Failing;

impl Read for Failing {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("the disk went away"))
    }
}

#[test]
fn an_entry_that_cannot_be_opened_in_a_changed_archive_reports_the_change() -> TestResult {
    let (_folder, path, index) = opened()?;
    emptied(&path)?;

    let mut delivered: Option<ReadResult> = None;
    read(
        &index,
        0,
        |result| {
            delivered = Some(result);
            true
        },
        &|| {},
    );

    assert!(
        matches!(delivered, Some(Err(Unread::ArchiveChanged))),
        "archive change, not a read failure: {delivered:?}"
    );
    Ok(())
}

#[test]
fn a_read_error_while_converting_in_a_changed_archive_reports_the_change() -> TestResult {
    let (_folder, path, index) = opened()?;
    let (mut preview, job) = XmlPreview::pending();
    emptied(&path)?;

    job.run(&index, (b"CryXmlB\0", 4096, None), &mut Failing, &|| {});
    preview.poll();

    assert!(
        matches!(preview.status(), XmlStatus::ArchiveChanged),
        "archive change, not a conversion failure: {:?}",
        preview.status()
    );
    Ok(())
}

#[test]
fn a_read_error_while_converting_in_an_unchanged_archive_names_the_error() -> TestResult {
    let (_folder, _path, index) = opened()?;
    let (mut preview, job) = XmlPreview::pending();

    job.run(&index, (b"CryXmlB\0", 4096, None), &mut Failing, &|| {});
    preview.poll();

    assert!(
        matches!(preview.status(), XmlStatus::Failed(Some(reason)) if reason.contains("the disk went away")),
        "failure: {:?}",
        preview.status()
    );
    Ok(())
}
