// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::NonZeroUsize;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_dataforge::{DataForge, Record, RecordTree, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{LoadoutRecordCache, LoadoutRecordReader, retained_bytes};
use crate::resolver_observation::alias_fixture;
use crate::{
    Entities, LoadoutPathObserver, LoadoutReadEvent, LoadoutReadObserver, LoadoutResolver,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn shared_record_cache_keeps_physical_aliases_foreign_owners_and_fifo_separate() -> TestResult {
    let bytes = alias_fixture::database();
    let forge = DataForge::open(&bytes)?;
    let foreign = DataForge::open(&bytes)?;
    let first = forge.record(0).ok_or("first physical alias")?;
    let second = forge.record(1).ok_or("second physical alias")?;
    assert_eq!(
        first.guid(),
        second.guid(),
        "fixture preserves a real GUID tie"
    );
    let cache = LoadoutRecordCache::new(&forge, NonZeroUsize::MIN, usize::MAX);
    let original = cache.read(&first)?.ok_or("first canonical tree")?;
    let reused = cache.read(&first)?.ok_or("retained canonical tree")?;
    assert!(
        Arc::ptr_eq(&original, &reused),
        "reuse the exact allocated tree"
    );
    let foreign = foreign.record(0).ok_or("not the original physical alias")?;
    assert!(
        cache.read(&foreign)?.is_none(),
        "even identical bytes in another forge are foreign"
    );
    let other = cache.read(&second)?.ok_or("second canonical tree")?;
    assert!(
        other.is_from(&second) && !other.is_from(&first),
        "the physical alias is its own cache key"
    );
    let rebuilt = cache.read(&first)?.ok_or("rebuilt canonical tree")?;
    assert!(
        !Arc::ptr_eq(&original, &rebuilt),
        "count limit evicts the oldest tree"
    );
    assert_eq!(
        original.as_ref(),
        rebuilt.as_ref(),
        "eviction preserves canonical values"
    );
    Ok(())
}

#[test]
fn shared_record_cache_skips_unreadable_values_and_oversized_trees() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("EntityClassDefinition").field("bad", TestType::Bool))
        .record(
            TestRecord::new(
                1,
                "EntityClassDefinition",
                "EntityClassDefinition.Broken",
                "fixture/broken.xml",
            )
            .value("bad", TestValue::Raw(vec![2])),
        )
        .record(TestRecord::new(
            2,
            "EntityClassDefinition",
            "EntityClassDefinition.Usable",
            "fixture/usable.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let failed = forge.record(0).ok_or("owner with an unreadable value")?;
    let usable = forge.record(1).ok_or("readable owner")?;
    let cache = LoadoutRecordCache::new(&forge, NonZeroUsize::MIN, usize::MAX);
    let first = cache.read(&failed)?.ok_or("partial canonical tree")?;
    let second = cache
        .read(&failed)?
        .ok_or("same partial canonical tree rebuilt")?;
    assert!(
        matches!(first.field("bad"), Some(Value::Error(_))),
        "actual unreadable original field goes through"
    );
    assert!(
        !Arc::ptr_eq(&first, &second),
        "a tree with a failed original value does not enter shared reuse"
    );
    let cache = LoadoutRecordCache::new(&forge, NonZeroUsize::MIN, 1);
    let first = cache
        .read(&usable)?
        .ok_or("readable tree past byte limit")?;
    let second = cache.read(&usable)?.ok_or("readable tree rebuilt")?;
    assert!(
        !Arc::ptr_eq(&first, &second),
        "byte pressure bypasses the cache without refusing the read"
    );
    Ok(())
}

#[test]
fn seeding_keeps_original_trees_without_evicting_at_either_limit() -> TestResult {
    let bytes = alias_fixture::database();
    let forge = DataForge::open(&bytes)?;
    let foreign = DataForge::open(&bytes)?;
    let first = forge.record(0).ok_or("first physical alias")?;
    let second = forge.record(1).ok_or("second physical alias")?;
    let foreign_first = foreign.record(0).ok_or("foreign physical alias")?;
    let first_bytes = retained_bytes(&first.tree()?).ok_or("readable first alias")?;
    let limits = [
        (NonZeroUsize::MIN, usize::MAX),
        (NonZeroUsize::MIN.saturating_add(1), first_bytes + 1),
    ];
    for (entries, bytes) in limits {
        let mut cache = LoadoutRecordCache::new(&forge, entries, bytes);
        assert!(
            !cache.try_seed(foreign_first.tree()?),
            "a foreign tree cannot enter the cache"
        );
        let tree = first.tree()?;
        let original_fields = tree.fields().as_ptr();
        assert!(
            cache.try_seed(tree),
            "the first admitted original tree fits"
        );
        assert!(
            !cache.try_seed(second.tree()?),
            "seeding never evicts existing trees"
        );
        assert!(
            cache.seed_saturated,
            "the first limit stops further seeding"
        );
        let seeds = cache.entries.get_mut().map_err(|_| "seeded cache")?;
        let retained = seeds
            .get(first.index())
            .ok_or("first admitted original tree")?;
        assert!(
            std::ptr::eq(retained.fields().as_ptr(), original_fields),
            "seeding retains the already decoded allocation from the original owner"
        );
        assert!(
            seeds.get(second.index()).is_none(),
            "seeding never evicts the earlier tree"
        );
    }
    Ok(())
}

struct Observer<'tree> {
    expected: &'tree RecordTree<'tree>,
    reads: AtomicUsize,
    paths: AtomicUsize,
}

impl Observer<'_> {
    fn read_original(&self, record: &Record<'_>, tree: &RecordTree<'_>, count: &AtomicUsize) {
        if record.index() == self.expected.record_index() {
            assert!(
                tree.is_from(record),
                "the observer gets its original physical Record"
            );
            assert!(
                std::ptr::addr_eq(tree, self.expected),
                "the observer receives the actual shared canonical tree"
            );
            count.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl LoadoutReadObserver for Observer<'_> {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        if let LoadoutReadEvent::CanonicalRead {
            record,
            outcome: Ok(tree),
        } = event
        {
            self.read_original(record, tree, &self.reads);
        }
        ControlFlow::Continue(())
    }

    fn with_record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        self.read_original(record, tree, &self.paths);
        parse(None)
    }
}

#[derive(Clone, Copy)]
enum Resolve {
    Prefetched,
    Serial,
}

#[test]
fn shared_tree_emits_canonical_and_path_evidence_for_each_fresh_resolver() -> TestResult {
    let bytes = alias_fixture::database();
    let forge = DataForge::open(&bytes)?;
    let root = forge.record(2).ok_or("original resolver root")?;
    let entries = NonZeroUsize::new(8).ok_or("nonzero fixture cache limit")?;
    let mut cache = LoadoutRecordCache::new(&forge, entries, usize::MAX);
    assert!(
        cache.try_seed(root.tree()?),
        "the original root fits in the cache"
    );
    let tree = cache
        .entries
        .get_mut()
        .map_err(|_| "original seeded tree cache")?
        .get(root.index())
        .cloned()
        .ok_or("original seeded tree")?;
    let entities = Arc::new(Entities::new(&forge));
    for route in [Resolve::Prefetched, Resolve::Serial] {
        let observer = Arc::new(Observer {
            expected: &tree,
            reads: AtomicUsize::new(0),
            paths: AtomicUsize::new(0),
        });
        let mut resolver =
            LoadoutResolver::with_observed_entities(Arc::clone(&entities), observer.clone())
                .with_record_cache(&cache);
        match route {
            Resolve::Prefetched => {
                resolver.resolve(&root)?;
            }
            Resolve::Serial => {
                assert!(
                    resolver.spec_record(&root).is_ok(),
                    "serial canonical projection completes"
                );
            }
        }
        assert_eq!(
            observer.reads.load(Ordering::Relaxed),
            1,
            "the active owner sees a canonical result, including on a cache hit"
        );
        assert_eq!(
            observer.paths.load(Ordering::Relaxed),
            1,
            "the active owner processes the original Record paths"
        );
    }
    Ok(())
}
