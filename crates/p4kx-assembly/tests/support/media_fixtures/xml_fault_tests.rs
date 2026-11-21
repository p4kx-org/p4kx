// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities, LoadoutDocumentKind, LoadoutFileError};
use p4kx_extraction::{Archive, DataForge};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout};

use super::observer::Fault;
use super::{Index, tests};

#[test]
fn actual_xml_read_and_parse_faults_preserve_the_same_physical_entry() -> super::Result<()> {
    for encrypted in [true, false] {
        let (_directory, archive) = archive(encrypted)?;
        let files = ArchiveFiles::new(&archive);
        let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
            .record(
                TestEntity::new(1, "Family_Preferred", "synthetic/root.xml")
                    .loadout(TestLoadout::Xml("Scripts/Synthetic.xml".to_owned()))
                    .record(),
            )
            .build();
        let forge = DataForge::open(&bytes)?;
        let index = Index::new(Arc::new(Entities::new(&forge)), &files)?;
        let error = index
            .choose(
                &archive,
                &tests::case(&["Family_Preferred"], tests::OPTIONAL),
            )
            .err()
            .ok_or("actual source document failure was accepted")?;
        let expected = archive
            .entries()
            .first()
            .ok_or("source Entry absent")?
            .path();
        verify(tests::fault(error.as_ref())?, expected, encrypted)?;
    }
    Ok(())
}

fn archive(encrypted: bool) -> super::Result<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let entry = TestEntry::stored("Data/Scripts/Synthetic.xml", b"not XML".to_vec());
    let entry = if encrypted { entry.encrypted() } else { entry };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    Ok((directory, Archive::open(path)?))
}

fn verify(fault: &Fault, expected: &str, encrypted: bool) -> super::Result<()> {
    match fault {
        Fault::FileRead {
            entry_index,
            path,
            kind,
            source,
        } if encrypted => {
            assert_eq!(
                *entry_index,
                Some(0),
                "byte failure retains the physical Entry index"
            );
            assert_eq!(
                path, expected,
                "byte failure retains the actually resolved source path internally"
            );
            assert_eq!(
                *kind,
                LoadoutDocumentKind::XmlLoadout,
                "the original document purpose is preserved"
            );
            assert!(
                matches!(source, LoadoutFileError::Unreadable { .. }),
                "actual unreadable bytes remain typed"
            );
        }
        Fault::FileProjection {
            entry_index,
            path,
            kind,
            reason,
        } if !encrypted => {
            assert_eq!(
                *entry_index,
                Some(0),
                "parse failure retains the same physical Entry index"
            );
            assert_eq!(
                path, expected,
                "parse failure retains the resolved path internally"
            );
            assert_eq!(
                *kind,
                LoadoutDocumentKind::XmlLoadout,
                "parse purpose is not guessed from text"
            );
            assert!(
                !reason.is_empty(),
                "the original opaque parse detail is retained internally"
            );
        }
        _ => return Err("read and parse fault stages were conflated".into()),
    }
    Ok(())
}
