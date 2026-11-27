// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with one plain and two encrypted Entries, for the tests of the kept results.

use std::error::Error;
use std::io::Read;
use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

/// The path of the plain Entry.
pub(crate) const PLAIN: &str = "Data\\plain.xml";
/// The path of the encrypted Entry.
pub(crate) const SECRET: &str = "Data\\secret.xml";

/// The path of a second encrypted Entry.
pub(crate) const OTHER: &str = "Data\\other.xml";

/// Writes the Archive into `dir` and opens it without a key.
pub(crate) fn archive(dir: &Path) -> Result<Archive, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(PLAIN, b"<plain/>".to_vec()))
        .entry(TestEntry::stored(SECRET, b"<secret/>".to_vec()).encrypted())
        .entry(TestEntry::stored(OTHER, b"<other/>".to_vec()).encrypted())
        .write_to(&path)?;
    Ok(Archive::open(&path)?)
}

/// Asks for the Entry at `path`, whether or not the key lets it be read.
pub(crate) fn read(archive: &Archive, path: &str) {
    if let Some(entry) = archive.entries().iter().find(|entry| entry.path() == path)
        && let Ok(mut reader) = archive.open_entry(entry)
    {
        let _ = reader.read_to_end(&mut Vec::new());
    }
}
