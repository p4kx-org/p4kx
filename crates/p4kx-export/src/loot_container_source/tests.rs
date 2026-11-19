// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

use crate::ExportDiagnosticCode;
use crate::export_ledger::SourceDisposition;
use crate::loot_unreadable::Unreadable;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn opened(
    container: &TestContainer,
) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("loot.socpak", container.build()))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

#[test]
fn malformed_xml_does_not_hide_a_later_readable_entity() -> TestResult {
    let container = TestContainer::new()
        .entry(TestEntry::stored("entdata/bad.entxml", "not XML"))
        .entry(TestEntry::stored(
            "entdata/good.entxml",
            r#"<Entity objectContainer="next.socpak"/>"#,
        ));
    let (_folder, archive) = opened(&container)?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        super::read(&archive, 0, None, capture)
    })?;
    assert!(
        matches!(read.unreadable, Some(Unreadable::EntityXml(_))),
        "parse failure is reported"
    );
    assert_eq!(read.links.len(), 1, "later valid source remains available");
    let [outer, malformed, valid] = read.ledger.entries.as_slice() else {
        return Err("all three actual operations expected".into());
    };
    assert_eq!(
        outer.disposition,
        SourceDisposition::Projected,
        "container opened"
    );
    assert_eq!(
        malformed.disposition,
        SourceDisposition::ProjectionFailed,
        "bytes were read but XML was invalid"
    );
    assert_eq!(
        valid.disposition,
        SourceDisposition::Projected,
        "later selected file was read"
    );
    Ok(())
}

#[test]
fn a_malformed_config_keeps_other_declared_links_with_field_diagnostics() -> TestResult {
    let container = TestContainer::new().entry(TestEntry::stored(
        "entdata/one.entxml",
        r#"<Entity objectContainer="next.socpak" multiConfigRef="invalid"/>"#,
    ));
    let (_folder, archive) = opened(&container)?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        super::read(&archive, 0, None, capture)
    })?;
    let [(_, link)] = read.links.as_slice() else {
        return Err("one partially projected source expected".into());
    };
    assert_eq!(
        link.value().container.as_deref(),
        Some("next.socpak"),
        "valid link survives"
    );
    assert!(
        link.value().malformed_config,
        "invalid populated GUID is explicit"
    );
    let [_, source] = read.ledger.entries.as_slice() else {
        return Err("outer plus source expected".into());
    };
    assert_eq!(
        source.disposition,
        SourceDisposition::ProjectionFailed,
        "retained partial data is not complete"
    );
    assert_eq!(
        source.diagnosis.code,
        Some(ExportDiagnosticCode::MalformedSource),
        "stable field code"
    );
    assert_eq!(
        source.diagnosis.accounting_field(source.path.as_str()),
        Some("multiConfigRef"),
        "actual field location"
    );
    Ok(())
}

#[test]
fn a_large_source_keeps_the_link_after_the_full_entity_document() -> TestResult {
    let padding = "x".repeat(1024 * 1024);
    let xml = format!("<!--{padding}--><Entity objectContainer=\"next.socpak\"/>");
    let container = TestContainer::new().entry(TestEntry::deflated("entdata/full.entxml", xml));
    let (_folder, archive) = opened(&container)?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        super::read(&archive, 0, None, capture)
    })?;
    let [(_, link)] = read.links.as_slice() else {
        return Err("the original link at the end of the source must survive".into());
    };
    assert_eq!(link.value().container.as_deref(), Some("next.socpak"));
    assert_eq!(read.unreadable, None);
    let [_, source] = read.ledger.entries.as_slice() else {
        return Err("outer container and the complete Entity source".into());
    };
    assert_eq!(source.disposition, SourceDisposition::Projected);
    Ok(())
}

#[test]
fn an_overlong_entity_source_cannot_publish_its_valid_prefix() -> TestResult {
    let xml = r#"<Entity objectContainer="next.socpak"/>"#;
    let entry = TestEntry::deflated("entdata/overlong.entxml", format!("{xml}broken"))
        .with_uncompressed_size(u64::try_from(xml.len())?);
    let (_folder, archive) = opened(&TestContainer::new().entry(entry))?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        super::read(&archive, 0, None, capture)
    })?;
    assert_eq!(
        read.links.len(),
        0,
        "an underdeclared stream yields no link"
    );
    assert_eq!(read.unreadable, Some(Unreadable::EntityFile));
    let [_, source] = read.ledger.entries.as_slice() else {
        return Err("outer container and the failed Entity source".into());
    };
    assert_eq!(source.disposition, SourceDisposition::ReadFailed);
    Ok(())
}

#[test]
fn a_checksum_mismatch_cannot_supply_an_authored_selector() -> TestResult {
    let xml = r#"<Entity objectContainer="next.socpak"/>"#;
    let entry = TestEntry::stored("entdata/damaged.entxml", xml).with_crc32(0);
    let (_folder, archive) = opened(&TestContainer::new().entry(entry))?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        super::read(&archive, 0, None, capture)
    })?;
    assert!(
        read.links.is_empty(),
        "a corrupt payload supplies no selector"
    );
    assert_eq!(read.unreadable, Some(Unreadable::EntityFile));
    let [_, source] = read.ledger.entries.as_slice() else {
        return Err("outer container and the failed Entity source".into());
    };
    assert_eq!(source.disposition, SourceDisposition::ReadFailed);
    Ok(())
}

#[test]
fn duplicate_paths_preserve_distinct_physical_entry_indices() -> TestResult {
    let container = TestContainer::new()
        .entry(TestEntry::stored("entdata/same.entxml", "<Entity/>"))
        .entry(TestEntry::stored("entdata/same.entxml", "invalid"));
    let (_folder, archive) = opened(&container)?;
    let read = crate::catalog_read::original::with_entry_capture(&archive, |capture| {
        super::read(&archive, 0, None, capture)
    })?;
    let [_, first, second] = read.ledger.entries.as_slice() else {
        return Err("both physical source Entries expected".into());
    };
    assert_eq!(first.path, second.path, "same authored path");
    assert_eq!(
        first.container_entry_index,
        Some(0),
        "first physical source"
    );
    assert_eq!(
        second.container_entry_index,
        Some(1),
        "second physical source"
    );
    assert_eq!(
        first.disposition,
        SourceDisposition::Projected,
        "valid empty XML"
    );
    assert_eq!(
        second.disposition,
        SourceDisposition::ProjectionFailed,
        "different outcome cannot collapse by path"
    );
    Ok(())
}
