// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Physical DataForge origins retained through Archive, file and shared worker reads.

use std::error::Error;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;

use p4kx_extraction::{
    Archive, DataForgeEntry, DataForgeOrigin, DataForgeSource, EntryOrigin, LoadedDataForge,
    SourceEncryption,
};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Fixture"))
        .record(TestRecord::new(
            1,
            "Fixture",
            "Fixture.One",
            "fixture/one.xml",
        ))
        .build()
}

fn archive(builder: &ArchiveBuilder) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture.p4k");
    builder.write_to(&path)?;
    Ok((
        directory,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn origin(value: &DataForgeOrigin) -> TestResult<&EntryOrigin> {
    if let DataForgeOrigin::Archive(origin) = value {
        Ok(origin)
    } else {
        Err("Archive origin missing".into())
    }
}

fn child(archive: &Archive) -> TestResult<Archive> {
    let entry = archive.entries().first().ok_or("container Entry missing")?;
    Ok(archive.open_container(entry)?)
}

fn container(path: &str, bytes: Vec<u8>, flag: u16) -> TestEntry {
    let entry = TestEntry::stored(path, bytes);
    if flag == 0 {
        entry
    } else {
        entry.encrypted().with_encryption_field(flag)
    }
}

fn nested(outer: u16, middle: u16) -> ArchiveBuilder {
    let leaf = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database()))
        .build();
    let middle = ArchiveBuilder::new()
        .entry(container("Data\\middle.socpak", leaf, middle))
        .build();
    ArchiveBuilder::new().entry(container("Data\\outer.socpak", middle, outer))
}

#[test]
fn containing_entries_survive_without_being_selected_as_dataforge() -> TestResult {
    let cases = [
        (0, 0, SourceEncryption::Unencrypted),
        (1, 0, SourceEncryption::Encrypted),
        (7, 0, SourceEncryption::Unknown),
        (7, 1, SourceEncryption::Encrypted),
    ];
    for (outer_flag, middle_flag, expected) in cases {
        let (_directory, root) = archive(&nested(outer_flag, middle_flag))?;
        let middle = child(&root)?;
        let leaf = child(&middle)?;
        let read = DataForgeSource::Archive(&leaf).read()?;
        assert_eq!(read.open()?.record_count(), 1);
        assert_eq!(read.origin().encryption(), expected);
        assert!(
            read.origin().matches_archive(&leaf),
            "the actual containing view matches"
        );
        assert!(
            !read.origin().matches_archive(&root),
            "a root view is not its contained view"
        );
        let physical = origin(read.origin())?;
        assert_eq!(physical.entry().path(), "Data\\Game2.dcb");
        assert_eq!(physical.entry().encryption(), SourceEncryption::Unencrypted);
        let ancestors: Vec<_> = physical
            .ancestors()
            .map(|step| step.map(|entry| (entry.index(), entry.path())))
            .collect();
        assert_eq!(
            ancestors,
            [
                Some((0, "Data\\outer.socpak")),
                Some((0, "Data\\middle.socpak"))
            ],
            "complete root-to-leaf containing Entry order"
        );
    }
    Ok(())
}

#[test]
fn duplicate_container_paths_preserve_their_distinct_admitted_indices() -> TestResult {
    let inner = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database()))
        .build();
    let builder = ArchiveBuilder::new()
        .entry(TestEntry::stored("same.socpak", inner.clone()))
        .entry(TestEntry::stored("same.socpak", inner).encrypted());
    let (_directory, root) = archive(&builder)?;
    for (index, entry) in root.entries().iter().enumerate() {
        let child = root.open_container(entry)?;
        let read = DataForgeEntry::read(&child)?;
        let physical = origin(read.origin())?;
        let containing = physical
            .ancestors()
            .next()
            .flatten()
            .ok_or("ancestor missing")?;
        assert_eq!(containing.index(), index);
        assert_eq!(containing.path(), "same.socpak");
        let expected = if index == 0 {
            SourceEncryption::Unencrypted
        } else {
            SourceEncryption::Encrypted
        };
        assert_eq!(physical.encryption(), expected);
    }
    Ok(())
}

#[test]
fn successful_reads_keep_unknown_and_duplicate_source_declarations() -> TestResult {
    let unknown = TestEntry::stored("Data\\Game2.dcb", database())
        .encrypted()
        .with_encryption_field(7);
    let duplicate = TestEntry::stored("Data\\Game2.dcb", database())
        .encrypted()
        .with_encryption_field(0)
        .with_empty_encryption_blocks(1);
    for entry in [unknown, duplicate] {
        let (_directory, archive) = archive(&ArchiveBuilder::new().entry(entry))?;
        let read = DataForgeEntry::read(&archive)?;
        assert_eq!(
            read.open()?.record_count(),
            1,
            "read success does not establish source state"
        );
        assert_eq!(
            read.origin().encryption(),
            SourceEncryption::Unknown,
            "external file bytes do not establish original P4K encryption"
        );
        let source = origin(read.origin())?.entry();
        let actual = DataForgeEntry::locate(&archive).ok_or("DataForge Entry missing")?;
        assert_eq!(
            source.metadata().encryption_declarations(),
            actual.metadata().encryption_declarations()
        );
        let loaded = LoadedDataForge::load(&archive)?;
        assert_eq!(loaded.origin().encryption(), SourceEncryption::Unknown);
        assert_eq!(
            loaded.with_origin(|forge, origin| (forge.record_count(), origin.encryption()))?,
            (1, SourceEncryption::Unknown)
        );
    }
    Ok(())
}

#[test]
fn archive_and_file_routes_preserve_different_origin_knowledge_for_identical_bytes() -> TestResult {
    let bytes = database();
    let builder = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", bytes.clone()).encrypted());
    let (directory, archive) = archive(&builder)?;
    let file = directory.path().join("Game2.dcb");
    fs::write(&file, bytes)?;
    let from_archive = DataForgeSource::Archive(&archive).read()?;
    let from_file = DataForgeSource::File(&file).read()?;
    assert_eq!(
        from_archive.open()?.record_count(),
        from_file.open()?.record_count()
    );
    assert_eq!(
        from_archive.origin().encryption(),
        SourceEncryption::Encrypted
    );
    check_file_snapshot(&from_file, &file)?;
    assert!(
        !from_file.origin().matches_archive(&archive),
        "File is a distinct input kind"
    );
    let loaded = DataForgeSource::File(&file).load()?;
    let retained = loaded.with_origin(|_forge, origin| origin.clone())?;
    let (DataForgeOrigin::File(cached), DataForgeOrigin::File(worker)) =
        (loaded.origin(), &retained)
    else {
        return Err("File origin was lost by worker/cache".into());
    };
    assert!(
        cached.is_same_input(worker),
        "worker and cached handle share the observed File input"
    );
    assert!(
        !format!("{cached:?}").contains(file.to_string_lossy().as_ref()),
        "debug output does not publish the machine host path"
    );
    assert_eq!(
        loaded.with_origin(|forge, origin| (forge.record_count(), origin.encryption()))?,
        (1, SourceEncryption::Unknown)
    );
    Ok(())
}

#[test]
fn cached_handle_and_real_worker_threads_keep_origin_after_archives_are_dropped() -> TestResult {
    let loaded = {
        let (_directory, root) = archive(&nested(1, 0))?;
        let middle = child(&root)?;
        let leaf = child(&middle)?;
        Arc::new(DataForgeSource::Archive(&leaf).load()?)
    };
    assert_eq!(loaded.origin().encryption(), SourceEncryption::Encrypted);
    let mut workers = Vec::new();
    for _ in 0..4 {
        let cached = Arc::clone(&loaded);
        workers.push(thread::spawn(move || {
            cached.with_origin(|forge, origin| (forge.record_count(), origin.clone()))
        }));
    }
    for worker in workers {
        let (records, retained) = worker.join().map_err(|_| "worker panicked")??;
        assert_eq!(records, 1);
        assert_eq!(retained.encryption(), SourceEncryption::Encrypted);
        assert_eq!(origin(&retained)?.ancestors().len(), 2);
        assert_eq!(origin(&retained)?.entry().path(), "Data\\Game2.dcb");
    }
    #[expect(
        clippy::redundant_closure_for_method_calls,
        reason = "a method pointer cannot satisfy the worker's higher-ranked input lifetime"
    )]
    let records_after_drop = loaded.with(|forge| forge.record_count())?;
    assert_eq!(
        records_after_drop, 1,
        "the existing caller Interface uses the same worker engine"
    );
    Ok(())
}

#[test]
fn failed_archive_reads_do_not_fabricate_a_dataforge_contributor() -> TestResult {
    let bytes = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database()).encrypted())
        .build();
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture.p4k");
    fs::write(&path, bytes)?;
    let archive = Archive::open(path)?;
    assert!(
        DataForgeEntry::read(&archive).is_err(),
        "the encrypted read requires a key"
    );
    let entry = DataForgeEntry::locate(&archive).ok_or("planned Entry missing")?;
    assert_eq!(
        archive
            .entry_origin(entry)
            .ok_or("declaration origin missing")?
            .encryption(),
        SourceEncryption::Encrypted,
        "attempt evidence remains available separately from a read result"
    );
    Ok(())
}

#[test]
fn an_owned_origin_cannot_be_rebound_after_its_archive_is_dropped() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database()))
        .write_to(&path)?;
    let read = {
        let observed = Archive::open(&path)?;
        let read = DataForgeEntry::read(&observed)?;
        assert!(
            read.origin().matches_archive(&observed),
            "the observed owner matches before drop"
        );
        read
    };
    let other = Archive::open(&path)?;
    let other_entry = DataForgeEntry::locate(&other).ok_or("other DataForge Entry missing")?;
    let retained = origin(read.origin())?;
    assert_eq!(retained.entry().index(), other_entry.index());
    assert_eq!(retained.entry().path(), other_entry.path());
    assert_eq!(retained.entry().metadata(), other_entry.metadata());
    assert!(
        !read.origin().matches_archive(&other),
        "identical paths, indices and metadata do not replace the retained root-input binding"
    );
    assert!(
        !retained.input_origin().is_same_input(other.input_origin()),
        "the live owned token prevents identity reuse after the original Archive drops"
    );
    Ok(())
}

#[test]
fn one_root_input_does_not_make_distinct_container_positions_interchangeable() -> TestResult {
    let inner = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database()))
        .build();
    let builder = ArchiveBuilder::new()
        .entry(TestEntry::stored("same.socpak", inner.clone()))
        .entry(TestEntry::stored("same.socpak", inner));
    let (_directory, root) = archive(&builder)?;
    let first = root.open_container(root.entries().first().ok_or("first container missing")?)?;
    let second = root.open_container(root.entries().get(1).ok_or("second container missing")?)?;
    let read = DataForgeEntry::read(&first)?;
    assert!(
        first.input_origin().is_same_input(second.input_origin()),
        "the root input is shared"
    );
    assert!(
        read.origin().matches_archive(&first),
        "the exact containing position matches"
    );
    assert!(
        !read.origin().matches_archive(&second),
        "different containing indices remain distinct"
    );
    Ok(())
}

fn check_file_snapshot(read: &DataForgeEntry, file: &Path) -> TestResult {
    let DataForgeOrigin::File(file_origin) = read.origin() else {
        return Err("the actual standalone File input was lost".into());
    };
    assert_eq!(
        file_origin.path(),
        std::path::absolute(file)?,
        "the observed input path stays absolute"
    );
    assert_eq!(
        file_origin.stamp().size(),
        fs::metadata(file)?.len(),
        "the opened input size is retained"
    );
    assert_eq!(
        file_origin.stamp().modified(),
        fs::metadata(file)?.modified().ok(),
        "the opened input modification observation is retained"
    );
    assert_eq!(
        read.origin().encryption(),
        SourceEncryption::Unknown,
        "external file bytes do not establish original P4K encryption"
    );
    let repeated = DataForgeEntry::read_file(file)?;
    let DataForgeOrigin::File(repeated_origin) = repeated.origin() else {
        return Err("repeated File input missing".into());
    };
    assert!(
        !file_origin.is_same_input(repeated_origin),
        "separate observations are not guessed equal"
    );
    assert!(
        file_origin.is_same_input(&file_origin.clone()),
        "the owned observation is shared"
    );
    Ok(())
}
