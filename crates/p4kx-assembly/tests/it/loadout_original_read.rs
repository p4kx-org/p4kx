// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_archive::Archive;
use p4kx_assembly::{
    ArchiveFiles, LoadoutFileError, LoadoutFileIdentity, LoadoutFileRead, LoadoutFileSource,
    LoadoutFiles,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use crate::loadout_original_controls_support::TestResult;

const PATH: &str = "synthetic/source.xml";
const BYTES: &[u8] = b"<Loadout/>";

struct DescriptiveProvider {
    claimed: Option<LoadoutFileSource>,
    reads: AtomicUsize,
    identities: AtomicUsize,
}

impl DescriptiveProvider {
    const fn new(claimed: Option<LoadoutFileSource>) -> Self {
        Self {
            claimed,
            reads: AtomicUsize::new(0),
            identities: AtomicUsize::new(0),
        }
    }
}

impl LoadoutFiles for DescriptiveProvider {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        assert_eq!(
            path, PATH,
            "the generic default preserves its configured target"
        );
        self.reads.fetch_add(1, Ordering::Relaxed);
        Ok(BYTES.to_vec())
    }

    fn identity(&self, _path: &str) -> Option<LoadoutFileIdentity<'_>> {
        self.identities.fetch_add(1, Ordering::Relaxed);
        Some(self.claimed.as_ref().map_or(
            LoadoutFileIdentity {
                entry_index: Some(0),
                path: PATH,
            },
            LoadoutFileSource::identity,
        ))
    }
}

struct Forward<'files> {
    inner: &'files dyn LoadoutFiles,
    reads: AtomicUsize,
}

impl LoadoutFiles for Forward<'_> {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        self.inner.read(path)
    }

    fn read_observed(&self, path: &str) -> LoadoutFileRead {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.inner.read_observed(path)
    }
}

fn archive(
    entries: Vec<TestEntry>,
) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let mut builder = ArchiveBuilder::new();
    for entry in entries {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    Ok((directory, Archive::open(&path)?))
}

#[test]
fn original_loadout_observed_admission_precedes_size_and_open_failures() -> TestResult {
    let size = p4kx_assembly::MAX_XML_LOADOUT_LEN + 1;
    let (_directory, archive) = archive(vec![
        TestEntry::stored("Data/synthetic/huge.xml", Vec::new())
            .with_uncompressed_size(size)
            .flagged_encrypted(),
        TestEntry::stored("Data/synthetic/locked.xml", BYTES).encrypted(),
    ])?;
    let files = ArchiveFiles::new(&archive);
    for (index, path) in ["synthetic/huge.xml", "synthetic/locked.xml"]
        .into_iter()
        .enumerate()
    {
        let read = files.read_observed(path);
        assert_admission(&archive, index, &read)?;
        if index == 0 {
            assert!(
                matches!(read.outcome(), Err(LoadoutFileError::TooLarge { size: found }) if *found == size),
                "size refusal occurs after original capture but before opening encrypted bytes"
            );
        } else {
            assert!(
                matches!(read.outcome(), Err(LoadoutFileError::Unreadable { .. })),
                "the actual missing-key failure retains the admitted source"
            );
        }
    }
    let missing = files.read_observed("synthetic/missing.xml");
    assert!(
        missing.source().entry_origin().is_none(),
        "missing paths cannot invent an admitted Entry"
    );
    assert!(
        matches!(missing.outcome(), Err(LoadoutFileError::NotFound)),
        "missing and unreadable remain distinct"
    );
    Ok(())
}

#[test]
fn original_loadout_generic_default_reads_once_and_never_consults_descriptive_identity()
-> TestResult {
    let provider = DescriptiveProvider::new(None);
    let read = (&provider as &dyn LoadoutFiles).read_observed(PATH);
    assert_eq!(
        provider.reads.load(Ordering::Relaxed),
        1,
        "the object-safe default reads exactly once"
    );
    assert_eq!(
        provider.identities.load(Ordering::Relaxed),
        0,
        "the default never re-looks-up a descriptor"
    );
    assert!(
        read.source().entry_origin().is_none(),
        "a claimed matching path/index creates no Archive authority"
    );
    assert_eq!(
        read.source().identity().entry_index,
        None,
        "unbound input has no admitted physical index"
    );
    assert_eq!(
        read.into_result()?,
        BYTES,
        "the generic default keeps the original bytes"
    );
    Ok(())
}

#[test]
fn original_loadout_forwarding_the_whole_genuine_read_preserves_its_original_pair() -> TestResult {
    let (_directory, archive) = archive(vec![TestEntry::stored(format!("Data/{PATH}"), BYTES)])?;
    let files = ArchiveFiles::new(&archive);
    let forward = Forward {
        inner: &files,
        reads: AtomicUsize::new(0),
    };
    let read = forward.read_observed(PATH);
    let original = read
        .source()
        .entry_origin()
        .ok_or("forwarding lost genuine original admission")?;
    assert!(
        archive.owns_origin(original),
        "whole-result forwarding preserves the actual owner"
    );
    assert!(
        original
            .input_origin()
            .is_same_input(archive.input_origin()),
        "the opaque original input remains identical"
    );
    assert_eq!(
        forward.reads.load(Ordering::Relaxed),
        1,
        "the adapter forwards one actual operation"
    );
    assert_eq!(
        read.into_result()?,
        BYTES,
        "the unchanged genuine read retains its original outcome"
    );
    Ok(())
}

#[test]
fn original_loadout_copied_genuine_source_descriptor_does_not_bind_arbitrary_provider_bytes()
-> TestResult {
    let (_directory, archive) = archive(vec![TestEntry::stored(format!("Data/{PATH}"), BYTES)])?;
    let files = ArchiveFiles::new(&archive);
    let genuine = files.read_observed(PATH);
    let original = genuine
        .source()
        .entry_origin()
        .ok_or("the genuine source is unbound")?;
    assert!(
        archive.owns_origin(original),
        "the negative starts from genuine original authority"
    );
    let provider = DescriptiveProvider::new(Some(genuine.source().clone()));
    let description = provider
        .identity(PATH)
        .ok_or("the copied public descriptor is absent")?;
    assert_eq!(
        description.entry_index,
        Some(0),
        "the provider really claims the admitted index"
    );
    let read = provider.read_observed(PATH);
    assert!(
        read.source().entry_origin().is_none(),
        "copying a genuine descriptor cannot bind new bytes"
    );
    assert_eq!(
        read.source().identity().entry_index,
        None,
        "the generic result stays explicitly unbound"
    );
    assert_eq!(
        provider.reads.load(Ordering::Relaxed),
        1,
        "the negative performs exactly its own read"
    );
    assert_eq!(
        provider.identities.load(Ordering::Relaxed),
        1,
        "only the explicit original control consulted identity"
    );
    assert_eq!(
        read.into_result()?,
        BYTES,
        "equal bytes still do not restore authority"
    );
    Ok(())
}

fn assert_admission(archive: &Archive, index: usize, read: &LoadoutFileRead) -> TestResult {
    let original = read
        .source()
        .entry_origin()
        .ok_or("original guarded admission was lost")?;
    let entry = archive
        .entries()
        .get(index)
        .ok_or("original directory slot is absent")?;
    assert!(
        archive.owns_origin(original),
        "the retained source belongs to the actual Archive"
    );
    assert_eq!(
        original.entry().index(),
        index,
        "the exact admitted physical slot is retained"
    );
    assert_eq!(
        original.entry().path(),
        entry.path(),
        "the original source path is retained"
    );
    assert_eq!(
        original.entry().metadata(),
        entry.metadata(),
        "guards retain the original directory facts"
    );
    Ok(())
}
