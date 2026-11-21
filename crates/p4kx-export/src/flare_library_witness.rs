// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read as _;

use p4kx_extraction::cryxml::{self, Element};
use p4kx_extraction::{Archive, Checksum};
use p4kx_testkit::real_archive_path;

use super::TestResult;
use crate::catalog_queries::flares::{FlareLibrarySelection, read_flare_libraries};
use crate::catalog_v3::flare_library::{FlareElement, FlareNodeRole, includes};
use crate::starmap_index;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and P4KX_FLARE_ENTRY and pass --ignored"]
fn installed_flare_library_matches_decoded_cryxml_occurrences() -> TestResult {
    let path = std::env::var("P4KX_FLARE_ENTRY")?;
    let archive = Archive::open(real_archive_path()?)?;
    let key = starmap_index::key(&path);
    let mut entries = archive
        .entries()
        .iter()
        .filter(|entry| includes(entry.path()) && starmap_index::key(entry.path()) == key);
    let entry = entries.next().ok_or("exact installed library source")?;
    assert!(
        entries.next().is_none(),
        "sample has a unique physical owner"
    );
    assert!(matches!(
        entry.checksum(),
        Checksum::Crc32(_) | Checksum::Crc32c(_)
    ));
    let mut bytes = Vec::new();
    let read = archive
        .open_entry_verified(entry)?
        .reject_overlong()
        .read_to_end(&mut bytes)?;
    assert_eq!(u64::try_from(read)?, entry.uncompressed_size());
    let document = cryxml::parse(&bytes)?;
    let snapshot = read_flare_libraries(&archive, FlareLibrarySelection::ArchivePath(&path), None)?;
    let library = snapshot.rows.first().ok_or("projected installed library")?;
    assert_eq!(snapshot.rows.len(), 1);
    compare(library.elements(), document.root())
}

fn compare(actual: &[FlareElement], root: Element<'_>) -> TestResult {
    let mut pending = vec![(root, None)];
    let mut nested = false;
    for (index, element) in actual.iter().enumerate() {
        let (expected, parent) = pending.pop().ok_or("a decoded occurrence")?;
        let authored = element.authored();
        assert_eq!(
            authored.name().value().map(String::as_str),
            Some(expected.name())
        );
        assert_eq!(authored.parent().value(), Some(&parent));
        compare_attributes(element, expected)?;
        if expected.name() == "FlareItem" && parent.is_some_and(|parent: u32| parent != 0) {
            assert_eq!(element.role().value(), Some(&FlareNodeRole::Component));
            nested = true;
        }
        let parent = Some(u32::try_from(index)?);
        let first = pending.len();
        pending.extend(expected.children().map(|element| (element, parent)));
        pending
            .get_mut(first..)
            .ok_or("expected child range")?
            .reverse();
    }
    assert!(
        pending.is_empty(),
        "every original CryXml node is represented"
    );
    assert!(nested, "the sample exercises a nested flare effect");
    Ok(())
}

fn compare_attributes(actual: &FlareElement, expected: Element<'_>) -> TestResult {
    let original: Vec<_> = expected
        .attributes()
        .map(|attribute| (attribute.name(), attribute.value()))
        .collect();
    let authored = actual
        .authored()
        .declarations()
        .iter()
        .map(|attribute| {
            let name = attribute
                .name()
                .value()
                .ok_or("authored declaration name")?;
            let value = attribute
                .value()
                .value()
                .value()
                .ok_or("authored declaration value")?;
            Ok((name.as_str(), value.as_str()))
        })
        .collect::<TestResult<Vec<_>>>()?;
    assert!(
        authored == original,
        "every original declaration and its order survive"
    );
    Ok(())
}
