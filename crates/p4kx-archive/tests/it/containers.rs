// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Opening containers nested in Entries through the public API.

use std::error::Error;
use std::io::Read;
use std::path::Path;

use p4kx_archive::{Archive, ArchiveError, Checksum, Entry, MAX_CONTAINER_DEPTH};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, test_key};

type TestResult = Result<(), Box<dyn Error>>;

const SOCPAK: &str = "Data\\ObjectContainers\\Ships\\room.socpak";

fn room() -> TestContainer {
    TestContainer::new()
        .entry(TestEntry::deflated(
            "room\\entdata\\1.entxml",
            "<Entity name=\"chair\"/>",
        ))
        .entry(TestEntry::stored("room\\room.xml", "<Room/>"))
}

fn open(dir: &Path, builder: &ArchiveBuilder) -> Result<Archive, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    builder.write_to(&path)?;
    Ok(Archive::open_with_key(&path, Some(test_key()))?)
}

fn entry<'a>(archive: &'a Archive, path: &str) -> Result<&'a Entry, Box<dyn Error>> {
    Ok(archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or_else(|| format!("no Entry {path}"))?)
}

fn read(archive: &Archive, path: &str) -> Result<String, Box<dyn Error>> {
    let mut text = String::new();
    archive
        .open_entry(entry(archive, path)?)?
        .read_to_string(&mut text)?;
    Ok(text)
}

fn paths(archive: &Archive) -> Vec<&str> {
    archive.entries().iter().map(Entry::path).collect()
}

#[test]
fn a_stored_container_lists_and_reads_its_entries() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(SOCPAK, room().build())),
    )?;

    let container = archive.open_container(entry(&archive, SOCPAK)?)?;

    assert_eq!(
        paths(&container),
        ["room\\entdata\\1.entxml", "room\\room.xml"],
        "the Entries of the container in directory order"
    );
    assert_eq!(
        read(&container, "room\\entdata\\1.entxml")?,
        "<Entity name=\"chair\"/>",
        "a deflated Entry reads like an Archive Entry"
    );
    assert_eq!(
        read(&container, "room\\room.xml")?,
        "<Room/>",
        "a stored one too"
    );
    Ok(())
}

#[test]
fn a_container_belongs_to_the_archive_file_and_names_its_location() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(SOCPAK, room().build())),
    )?;

    let container = archive.open_container(entry(&archive, SOCPAK)?)?;

    assert_eq!(container.path(), archive.path(), "the file it is read from");
    assert_eq!(container.file_len(), archive.file_len(), "that file's size");
    assert_eq!(
        container.location(),
        archive.path().join(SOCPAK),
        "the Archive followed by the Entry"
    );
    assert_eq!(
        archive.location(),
        archive.path(),
        "an Archive is its own location"
    );
    Ok(())
}

#[test]
fn container_entries_carry_the_crc_32_of_their_data() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(SOCPAK, room().build())),
    )?;

    let container = archive.open_container(entry(&archive, SOCPAK)?)?;

    assert_eq!(
        entry(&container, "room\\room.xml")?.checksum(),
        Checksum::Crc32(0x58BC_C3D8),
        "the plain ZIP CRC-32 of <Room/>"
    );
    assert_eq!(
        entry(&archive, SOCPAK)?.checksum(),
        Checksum::Crc32c(crc32c::crc32c(&room().build())),
        "Archive Entries keep their CRC-32C"
    );
    Ok(())
}

#[test]
fn a_compressed_or_encrypted_container_is_decoded_in_memory() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new()
            .entry(TestEntry::deflated("Data\\a.socpak", room().build()))
            .entry(TestEntry::zstd("Data\\b.socpak", room().build()).encrypted()),
    )?;

    for path in ["Data\\a.socpak", "Data\\b.socpak"] {
        let container = archive.open_container(entry(&archive, path)?)?;
        assert_eq!(read(&container, "room\\room.xml")?, "<Room/>", "{path}");
    }
    Ok(())
}

#[test]
fn containers_inside_containers_are_opened_the_same_way() -> TestResult {
    let dir = tempfile::tempdir()?;
    let outer = TestContainer::new()
        .entry(TestEntry::stored("ship\\ship.xml", "<Ship/>"))
        .container("ship\\modules\\room.socpak", &room())
        .entry(TestEntry::deflated(
            "ship\\modules\\deflated.socpak",
            room().build(),
        ));
    let archive = open(
        dir.path(),
        &ArchiveBuilder::v2().entry(TestEntry::stored(SOCPAK, outer.build())),
    )?;

    let ship = archive.open_container(entry(&archive, SOCPAK)?)?;
    for path in [
        "ship\\modules\\room.socpak",
        "ship\\modules\\deflated.socpak",
    ] {
        let inner = ship.open_container(entry(&ship, path)?)?;
        assert_eq!(read(&inner, "room\\room.xml")?, "<Room/>", "{path}");
        assert_eq!(
            inner.location(),
            archive.path().join(SOCPAK).join(path),
            "the location names every container on the way"
        );
    }
    Ok(())
}

#[test]
fn paths_with_forward_slashes_use_backslashes_like_archive_entries() -> TestResult {
    let dir = tempfile::tempdir()?;
    let container = TestContainer::new()
        .entry(TestEntry::stored("room/", ""))
        .entry(TestEntry::stored("room/room.xml", "<Room/>"));
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(SOCPAK, container.build())),
    )?;

    let container = archive.open_container(entry(&archive, SOCPAK)?)?;

    assert_eq!(
        paths(&container),
        ["room\\room.xml"],
        "folder Entries are left out, separators are backslashes"
    );
    Ok(())
}

fn container_error(
    archive: &Archive,
    path: &str,
) -> Result<(String, ArchiveError), Box<dyn Error>> {
    match archive.open_container(entry(archive, path)?) {
        Ok(_) => Err(format!("{path} opened as a container").into()),
        Err(ArchiveError::Container { entry, source, .. }) => Ok((entry, *source)),
        Err(other) => Err(format!("{path} failed without naming the Entry: {other:?}").into()),
    }
}

#[test]
fn a_corrupt_container_gives_an_error_naming_the_entry() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut damaged = room().build();
    let header = damaged
        .windows(4)
        .position(|window| window == [0x50, 0x4B, 0x01, 0x02])
        .ok_or("no central directory header")?;
    damaged[header + 2] = 0x09;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new()
            .entry(TestEntry::stored("Data\\text.socpak", "not a container"))
            .entry(TestEntry::stored("Data\\damaged.socpak", damaged)),
    )?;

    let (named, source) = container_error(&archive, "Data\\text.socpak")?;
    assert_eq!(named, "Data\\text.socpak", "the Entry is named");
    assert!(
        matches!(source, ArchiveError::NotAnArchive { .. }),
        "text is not an Archive: {source:?}"
    );

    let (named, source) = container_error(&archive, "Data\\damaged.socpak")?;
    assert_eq!(named, "Data\\damaged.socpak", "the Entry is named");
    assert!(
        matches!(source, ArchiveError::Corrupt { offset, .. } if offset == header as u64),
        "the damage is reported at its offset inside the container: {source:?}"
    );
    let message = archive
        .open_container(entry(&archive, "Data\\damaged.socpak")?)
        .err()
        .map_or_default(|error| error.to_string());
    assert!(
        message.contains("Data\\damaged.socpak"),
        "the message names the Entry: {message}"
    );
    Ok(())
}

#[test]
fn containers_nest_only_to_the_depth_limit() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut nested = room();
    for _ in 0..MAX_CONTAINER_DEPTH {
        nested = TestContainer::new().container("inner.socpak", &nested);
    }
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(SOCPAK, nested.build())),
    )?;

    let mut container = archive.open_container(entry(&archive, SOCPAK)?)?;
    for _ in 1..MAX_CONTAINER_DEPTH {
        container = container.open_container(entry(&container, "inner.socpak")?)?;
    }
    let (named, source) = container_error(&container, "inner.socpak")?;

    assert_eq!(named, "inner.socpak", "the Entry beyond the limit is named");
    assert!(
        source.to_string().contains("deeper"),
        "the depth limit is the reason: {source}"
    );
    Ok(())
}

#[test]
fn an_entry_that_inflates_beyond_the_ratio_limit_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let bomb = TestContainer::new()
        .entry(TestEntry::deflated("zeros.bin", vec![0; 4096]).with_uncompressed_size(1 << 40));
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(SOCPAK, bomb.build())),
    )?;

    let (_, source) = container_error(&archive, SOCPAK)?;

    assert!(
        source.to_string().contains("ratio"),
        "the compression ratio is the reason: {source}"
    );
    Ok(())
}

#[test]
fn entries_sharing_their_data_are_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let mut bytes = TestContainer::new()
        .entry(TestEntry::stored("a.bin", vec![7; 64]))
        .entry(TestEntry::stored("b.bin", vec![7; 64]))
        .build();
    let second = bytes
        .windows(4)
        .rposition(|window| window == [0x50, 0x4B, 0x01, 0x02])
        .ok_or("no second central directory header")?;
    bytes[second + 42..second + 46].copy_from_slice(&0_u32.to_le_bytes());
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(SOCPAK, bytes)),
    )?;

    let (_, source) = container_error(&archive, SOCPAK)?;

    assert!(
        source.to_string().contains("overlap"),
        "overlapping Entry data is the reason: {source}"
    );
    Ok(())
}

#[test]
fn a_stored_entry_whose_sizes_differ_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let container = TestContainer::new()
        .entry(TestEntry::stored("a.bin", vec![7; 64]).with_uncompressed_size(65));
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new()
            .entry(TestEntry::stored(SOCPAK, container.build()))
            .entry(
                TestEntry::stored("Data\\short.socpak", room().build()).with_uncompressed_size(1),
            ),
    )?;

    let (_, source) = container_error(&archive, SOCPAK)?;
    assert!(
        source.to_string().contains("stored"),
        "a stored Entry inside the container: {source}"
    );
    let (named, _) = container_error(&archive, "Data\\short.socpak")?;
    assert_eq!(named, "Data\\short.socpak", "the container Entry itself");
    Ok(())
}

#[test]
fn a_declared_size_larger_than_the_data_fails_while_reading() -> TestResult {
    let dir = tempfile::tempdir()?;
    let container =
        TestContainer::new().entry(TestEntry::deflated("a.xml", "<a/>").with_uncompressed_size(40));
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new().entry(TestEntry::stored(SOCPAK, container.build())),
    )?;
    let container = archive.open_container(entry(&archive, SOCPAK)?)?;

    let mut data = Vec::new();
    let result = container
        .open_entry(entry(&container, "a.xml")?)?
        .read_to_end(&mut data);

    assert!(
        result.is_err(),
        "the real size falls short of the declared one"
    );
    assert_eq!(data, b"<a/>", "only the real data is produced");
    Ok(())
}

#[test]
fn a_container_too_large_to_decode_in_memory_is_refused() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = open(
        dir.path(),
        &ArchiveBuilder::new()
            .entry(TestEntry::deflated(SOCPAK, room().build()).with_uncompressed_size(u64::MAX)),
    )?;

    let (named, source) = container_error(&archive, SOCPAK)?;

    assert_eq!(named, SOCPAK, "the Entry is named");
    assert!(
        source.to_string().contains("larger"),
        "the size limit is the reason: {source}"
    );
    Ok(())
}
