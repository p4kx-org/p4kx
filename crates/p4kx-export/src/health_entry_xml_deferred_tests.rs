// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::ExportAccountingError;
use crate::catalog_read::original::{EntryRead, EntryXml, with_entry_capture};
use crate::entry_source::EntrySource;

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn deferred_original_attributes_reject_another_capture_frame() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "source.xml",
            br#"<Child custom="original"/>"#.as_slice(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    with_entry_capture(&archive, |capture| {
        let entry = archive
            .entries()
            .first()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let source = EntrySource::outer(capture, &archive, entry)?;
        let read = EntryRead::read_all(source.receipt().clone(), &archive, entry, None)?
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        let xml = EntryXml::parse(&read, capture)?
            .map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        let original = xml.deferred_node(xml.root())?;
        with_entry_capture(&archive, |foreign| {
            assert!(
                original.descriptor(foreign).is_err(),
                "equal Archive cannot supply another read frame"
            );
            assert!(
                original.attributes(foreign).is_err(),
                "raw attributes retain their actual original binding"
            );
            assert!(
                original
                    .attribute(foreign, "custom", |value| Some(value.to_owned()))
                    .is_err(),
                "a copied value cannot recover another frame's field authority"
            );
            Ok(())
        })?;
        let attributes = original.attributes(capture)?;
        let [attribute] = attributes.as_slice() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        assert_eq!(
            attribute.value.value().map(String::as_str),
            Some("original"),
            "the admitted deferred attribute retains its actual original value"
        );
        Ok(())
    })?;
    Ok(())
}
