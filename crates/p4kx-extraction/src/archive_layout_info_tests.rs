// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::{Archive, ArchiveLayoutInfo};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const EXPECTED: &str = include_str!("../../p4kx-testkit/fixtures/archive-layout-v2-empty.json");

#[test]
fn cached_layout_info_matches_independent_v2_source_words() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("x", b""))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let actual = serde_json::to_value(ArchiveLayoutInfo::of(archive.metadata()))?;
    assert_eq!(
        actual,
        serde_json::from_str::<serde_json::Value>(EXPECTED)?,
        "independently authored v2 declarations"
    );
    assert!(
        actual.get("signatures").is_none(),
        "header summaries do not load per-Entry signatures"
    );
    Ok(())
}

#[test]
fn header_projection_needs_no_additional_source_io_or_lazy_row_validation() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("x", b""))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(0)?;
    assert_eq!(
        serde_json::to_value(ArchiveLayoutInfo::of(archive.metadata()))?,
        serde_json::from_str::<serde_json::Value>(EXPECTED)?,
        "cached header availability is not a fresh file or lazy-row validation result"
    );
    Ok(())
}

#[test]
fn cached_layout_origins_address_every_v2_value_without_reopening_source() -> TestResult {
    use crate::{FieldPath, SourceEncryptionState, SourceLocation};

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.p4k");
    ArchiveBuilder::v2()
        .entry(TestEntry::stored("unread.bin", b"data").encrypted())
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)?
        .set_len(0)?;

    let (layout, origins) = ArchiveLayoutInfo::project_with_origins(&archive, "/metadata")?;
    let value = serde_json::to_value(layout)?;
    assert_every_origin(&value, &origins, "/metadata", false)?;
    let version = origins
        .field_origins
        .get(&FieldPath::parse("/metadata/version")?)
        .ok_or("version origin")?;
    assert!(
        version.source_uses_refs().is_empty(),
        "contract version is tool-authored"
    );
    let sector = origins
        .field_origins
        .get(&FieldPath::parse("/metadata/v2/sectorSize")?)
        .ok_or("sector origin")?;
    let reference = sector.source_uses_refs().first().ok_or("source use")?;
    let source = origins
        .provenance
        .uses()
        .iter()
        .find(|usage| usage.id() == reference)
        .ok_or("closed provenance")?;
    let trailer = archive
        .metadata()
        .v2_directory()
        .ok_or("trailer")?
        .trailer();
    assert!(
        matches!(source.location(),
        SourceLocation::Subresource { namespace, indices, field }
        if namespace == "archiveV2Trailer"
            && indices.as_slice() == [trailer.offset(), trailer.offset() + trailer.len()]
            && field == "v2/sectorSize"),
        "sector size links to the observed trailer and its exact logical field"
    );
    assert_eq!(
        source.effective_encryption(),
        SourceEncryptionState::Unencrypted,
        "root directory remains plain despite encrypted sibling payload"
    );
    assert_eq!(
        origins.provenance.sources().len(),
        1,
        "only the root directory contributes"
    );
    Ok(())
}

#[test]
fn cached_layout_origins_keep_the_outer_encrypted_container_for_plain_inner_layout() -> TestResult {
    use p4kx_testkit::{TestContainer, test_key};

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.p4k");
    let leaf = TestContainer::new()
        .entry(TestEntry::stored("leaf.bin", b""))
        .build();
    let outer = ArchiveBuilder::new()
        .entry(TestEntry::stored("plain.socpak", leaf))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::deflated("encrypted.socpak", outer).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    let outer = archive.open_container(archive.entries().first().ok_or("outer Entry")?)?;
    let leaf = outer.open_container(outer.entries().first().ok_or("inner Entry")?)?;

    let (layout, origins) =
        ArchiveLayoutInfo::project_with_origins(&leaf, "/structuredContent/metadata")?;
    assert_every_origin(
        &serde_json::to_value(layout)?,
        &origins,
        "/structuredContent/metadata",
        true,
    )?;
    let inventory = serde_json::to_value(&origins.provenance)?;
    let sources = inventory
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .ok_or("physical sources")?;
    assert_eq!(sources.len(), 2, "outer and inner physical sources");
    assert!(
        sources.iter().any(|source| {
            source.get("storedEncryption") == Some(&serde_json::json!("unencrypted"))
                && source
                    .get("ancestors")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|parents| parents.len() == 1)
        }),
        "plain inner Container retains its encrypted ancestor"
    );
    Ok(())
}

#[test]
fn recovered_layout_absence_is_unknown_without_a_fabricated_header() -> TestResult {
    use crate::{FieldPath, OpenOptions};

    let directory = tempfile::tempdir()?;
    let path = directory.path().join("source.p4k");
    let mut data = ArchiveBuilder::new()
        .entry(TestEntry::stored("item.bin", b""))
        .build();
    let end = data
        .windows(4)
        .rposition(|bytes| bytes == b"PK\x05\x06")
        .ok_or("ZIP end")?;
    data.truncate(end);
    std::fs::write(&path, data)?;
    let archive = Archive::open_with(&path, OpenOptions::new().recover(true))?;
    let (layout, origins) = ArchiveLayoutInfo::project_with_origins(&archive, "/metadata")?;
    assert_eq!(
        serde_json::to_value(layout)?.get("layout"),
        Some(&serde_json::json!("recovered-unknown")),
        "recovered route retained"
    );
    for field in ["zip", "v2", "layout", "declarationState"] {
        let path = FieldPath::parse(&format!("/metadata/{field}"))?;
        let origin = origins
            .field_origins
            .get(&path)
            .ok_or("recovery field origin")?;
        assert!(
            origin.contains_encrypted_source().is_none(),
            "no proved layout source for {field}"
        );
        assert!(
            !origin.source_set_complete(),
            "recovery cannot establish the original field {field}"
        );
    }
    Ok(())
}

fn assert_every_origin(
    value: &serde_json::Value,
    origins: &crate::ArchiveLayoutOrigins,
    pointer: &str,
    encrypted: bool,
) -> TestResult {
    let path = crate::FieldPath::parse(pointer)?;
    let origin = origins.field_origins.get(&path).ok_or("immediate origin")?;
    let is_version = pointer.ends_with("/version");
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(encrypted && !is_version),
        "field {pointer}"
    );
    assert!(
        origin.source_set_complete(),
        "contributor set for {pointer}"
    );
    for reference in origin.source_uses_refs() {
        let observed = origins
            .provenance
            .uses()
            .iter()
            .find(|usage| usage.id() == reference)
            .ok_or("registered use")?;
        assert!(
            origins
                .provenance
                .sources()
                .iter()
                .any(|source| source.id() == observed.source()),
            "physical owner of {pointer}"
        );
    }
    if let Some(children) = value.as_object() {
        for (key, value) in children {
            assert_every_origin(value, origins, &format!("{pointer}/{key}"), encrypted)?;
        }
    }
    Ok(())
}
