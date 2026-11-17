// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, SourceLocation};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::catalog_read::original::{EntryRead, EntryXml, with_entry_capture};
use crate::entry_source::EntrySource;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn original_attribute_iteration_keeps_ordinals_and_rejects_foreign_frames() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new().entry(TestEntry::stored("source.xml", r#"<Entity ObjectContainer="source.socpak" multiConfigRef="00000000-0000-0000-0000-000000000000"/>"#)).write_to(&path)?;
    let archive = Archive::open(&path)?;
    with_entry_capture(&archive, |capture| {
        let entry = archive.entries().first().expect("one original Entry");
        let source = EntrySource::outer(capture, &archive, entry)?;
        let read = EntryRead::read_all(source.receipt().clone(), &archive, entry, None)?
            .expect("complete original XML");
        let xml = EntryXml::parse(&read, capture)?.expect("valid original XML");
        let node = xml.node_source(xml.root())?.index();
        let mut seen = Vec::new();
        with_entry_capture(&archive, |foreign| {
            xml.visit_attributes(xml.root(), |attribute| {
                assert!(attribute.captured(foreign).is_err(), "an actual attribute cannot be rebound to another capture frame");
                let token = attribute.captured(capture)?;
                let expected = match attribute.ordinal() { 0 => "source.socpak", 1 => "00000000-0000-0000-0000-000000000000", _ => panic!("only two original attributes"), };
                assert_eq!(token.as_str(), expected, "the token retains its actual original value");
                let (uses, complete) = token.selector_inputs()?;
                assert!(complete, "the original exact attribute is readable");
                assert!(!uses.is_empty(), "each selected attribute has original source coordinates");
                for used in uses {
                    assert!(matches!(used.description().location(), SourceLocation::Subresource { namespace, indices, field }
                        if namespace == "xml" && indices.as_slice() == [u64::from(node), attribute.ordinal()] && field == &format!("@{}", attribute.name())),
                        "the token cites the same actual node and attribute ordinal");
                }
                seen.push((attribute.name().to_owned(), attribute.ordinal()));
                Ok(())
            })
        })?;
        assert_eq!(
            seen,
            [
                ("ObjectContainer".to_owned(), 0),
                ("multiConfigRef".to_owned(), 1)
            ],
            "each original occurrence is visited once in order with original name spelling"
        );
        Ok(())
    })?;
    Ok(())
}
