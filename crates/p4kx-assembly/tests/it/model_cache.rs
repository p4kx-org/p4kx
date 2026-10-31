// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The model cache answers a repeated read from memory, within its bound.

use crate::assembly_support::{TestResult, fixture};

use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{
    ArchiveModels, CachedModels, ModelCache, ModelFailure, ModelProvider, ProvidedModel,
};
use p4kx_extraction::Archive;
use p4kx_extraction::model::MaterialStatus;
use p4kx_testkit::{RIG_LOGIC_CHUNK, TestEntry, TestModel};

const PATH: &str = "Objects/Test/cube.cgf";

struct Counting<'a> {
    inner: ArchiveModels<'a>,
    calls: AtomicUsize,
}

impl ModelProvider for Counting<'_> {
    fn model(&self, path: &str, lod: u32) -> Result<ProvidedModel, ModelFailure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.inner.model(path, lod)
    }

    fn library(&self, entry: &str, name: &str) -> MaterialStatus {
        self.inner.library(entry, name)
    }
}

fn cube() -> Vec<TestEntry> {
    let model = TestModel::cube();
    vec![
        TestEntry::stored("Data\\Objects\\Test\\cube.cgf", model.cgf()),
        TestEntry::stored("Data\\Objects\\Test\\cube.cgfm", model.cgfm()),
    ]
}

#[test]
fn a_model_read_twice_is_read_from_the_archive_once() -> TestResult {
    let fixture = fixture(cube())?;
    let archive = Archive::open(&fixture.path)?;
    let source = Counting {
        inner: ArchiveModels::new(&archive),
        calls: AtomicUsize::new(0),
    };
    let cache = ModelCache::default();
    let provider = CachedModels::new(&source, &cache);

    let first = provider.model(PATH, 1)?;
    let second = provider.model(&PATH.to_uppercase().replace('/', "\\"), 1)?;

    assert_eq!(
        source.calls.load(Ordering::SeqCst),
        1,
        "the second is cached"
    );
    assert_eq!(first.model, second.model, "the same model");
    assert_eq!(first.entry, second.entry, "the same entry");
    assert_eq!(cache.len(), 1);
    assert!(cache.bytes() > 0, "the size is counted");
    Ok(())
}

#[test]
fn a_level_the_model_does_not_have_is_answered_from_the_level_it_came_out_at() -> TestResult {
    let fixture = fixture(cube())?;
    let archive = Archive::open(&fixture.path)?;
    let source = Counting {
        inner: ArchiveModels::new(&archive),
        calls: AtomicUsize::new(0),
    };
    let cache = ModelCache::default();
    let provider = CachedModels::new(&source, &cache);

    let high = provider.model(PATH, 3)?;
    let level = high.model.lods.first().map_or(0, |lod| lod.level);
    let between = provider.model(PATH, level)?;

    assert_eq!(between.model, high.model);
    assert_eq!(
        source.calls.load(Ordering::SeqCst),
        1,
        "{level} up to 3 is one read"
    );
    Ok(())
}

#[test]
fn a_level_above_the_one_read_is_read_again() -> TestResult {
    let fixture = fixture(cube())?;
    let archive = Archive::open(&fixture.path)?;
    let source = Counting {
        inner: ArchiveModels::new(&archive),
        calls: AtomicUsize::new(0),
    };
    let cache = ModelCache::default();
    let provider = CachedModels::new(&source, &cache);

    provider.model(PATH, 0)?;
    provider.model(PATH, 2)?;

    assert_eq!(
        source.calls.load(Ordering::SeqCst),
        2,
        "the read at 0 says nothing about level 2"
    );
    Ok(())
}

#[test]
fn a_model_above_the_bound_is_not_kept() -> TestResult {
    let fixture = fixture(cube())?;
    let archive = Archive::open(&fixture.path)?;
    let source = Counting {
        inner: ArchiveModels::new(&archive),
        calls: AtomicUsize::new(0),
    };
    let cache = ModelCache::new(1);
    let provider = CachedModels::new(&source, &cache);

    provider.model(PATH, 1)?;
    provider.model(PATH, 1)?;

    assert_eq!(source.calls.load(Ordering::SeqCst), 2, "read both times");
    assert!(cache.is_empty());
    assert_eq!(cache.bytes(), 0);
    Ok(())
}

#[test]
fn the_least_recently_used_model_goes_first() -> TestResult {
    let model = TestModel::cube();
    let mut entries = cube();
    entries.push(TestEntry::stored(
        "Data\\Objects\\Test\\two.cgf",
        model.cgf(),
    ));
    entries.push(TestEntry::stored(
        "Data\\Objects\\Test\\two.cgfm",
        model.cgfm(),
    ));
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let source = Counting {
        inner: ArchiveModels::new(&archive),
        calls: AtomicUsize::new(0),
    };
    let probe = ModelCache::default();
    CachedModels::new(&source, &probe).model(PATH, 1)?;
    let one = probe.bytes();
    source.calls.store(0, Ordering::SeqCst);
    let cache = ModelCache::new(one + one / 2);
    let provider = CachedModels::new(&source, &cache);

    provider.model(PATH, 1)?;
    provider.model("Objects/Test/two.cgf", 1)?;
    provider.model("Objects/Test/two.cgf", 1)?;
    provider.model(PATH, 1)?;

    assert_eq!(
        source.calls.load(Ordering::SeqCst),
        3,
        "cube, two, cube again"
    );
    assert_eq!(cache.len(), 1, "one fits");
    assert!(cache.bytes() <= one + one / 2);
    Ok(())
}

#[test]
fn the_rig_logic_chunk_of_a_model_counts_toward_the_bound() -> TestResult {
    let chunk = [7_u8; 4096];
    let size_of = |model: &TestModel| -> Result<usize, Box<dyn std::error::Error>> {
        let fixture = fixture(vec![
            TestEntry::stored(r"Data\Objects\Test\cube.cgf", model.cgf()),
            TestEntry::stored(r"Data\Objects\Test\cube.cgfm", model.cgfm()),
        ])?;
        let archive = Archive::open(&fixture.path)?;
        let cache = ModelCache::default();
        CachedModels::new(&ArchiveModels::new(&archive), &cache).model(PATH, 1)?;
        Ok(cache.bytes())
    };

    let plain = size_of(&TestModel::cube())?;
    let with_chunk = size_of(&TestModel::cube().with_chunk(RIG_LOGIC_CHUNK, chunk))?;

    assert_eq!(with_chunk - plain, chunk.len());
    Ok(())
}
