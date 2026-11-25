// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::NonZeroUsize;
use std::ops::ControlFlow;
use std::ptr;
use std::sync::{Arc, Mutex};

use p4kx_archive::Archive;
use p4kx_testkit::{self, ArchiveBuilder, TestEntry};

use super::ArchiveFiles;
use crate::loadout_file_trace::FileTrace;
use crate::resolver_observation::Observation;
use crate::{
    LoadoutFileError, LoadoutFileObservation, LoadoutFiles, LoadoutPath, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutRequest, xml,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const ROOT: &str = "synthetic/root.xml";
const OTHER: &str = "synthetic/other.xml";
const XML: &[u8] = b"<Loadout/>";
const BUDGET: usize = 4096;

#[test]
fn cached_xml_reuses_the_allocation_and_preserves_physical_admission() -> TestResult {
    let (_directory, archive) = archive([entry(ROOT, XML), entry(OTHER, XML)])?;
    let files = ArchiveFiles::new(&archive)
        .with_verified_entries()
        .with_xml_read_cache(NonZeroUsize::new(2).ok_or("fixture entry limit")?, BUDGET);
    let first = files.read_observed(ROOT);
    let alias = files.read_observed(r"DATA\SYNTHETIC\ROOT.XML");
    let other = files.read_observed(OTHER);

    let bytes = first.outcome().map_err(Clone::clone)?;
    assert!(
        ptr::eq(bytes, alias.outcome().map_err(Clone::clone)?),
        "path aliases reuse the same admitted Entry allocation"
    );
    assert!(
        first.source().is_same_admission(alias.source()),
        "a hit carries the original physical Entry"
    );
    assert!(
        !first.source().is_same_admission(other.source()),
        "equal bytes do not merge different Entries"
    );
    assert!(
        !ptr::eq(bytes, other.outcome().map_err(Clone::clone)?),
        "another physical Entry has independent storage"
    );
    let mut owned = alias.into_result()?;
    owned.clear();
    assert!(
        owned.is_empty(),
        "the ordinary owned payload can be cleared"
    );
    assert_eq!(
        bytes, XML,
        "the public owned payload cannot mutate a retained file"
    );
    assert_eq!(
        files.read(ROOT)?,
        XML,
        "the ordinary Vec reader stays compatible"
    );
    Ok(())
}

#[test]
fn cached_xml_obeys_entry_and_byte_limits() -> TestResult {
    let (_directory, archive) = archive([entry(ROOT, XML), entry(OTHER, XML)])?;
    for (entries, bytes) in [(1, BUDGET), (2, XML.len())] {
        let files = ArchiveFiles::new(&archive)
            .with_verified_entries()
            .with_xml_read_cache(
                NonZeroUsize::new(entries).ok_or("fixture entry limit")?,
                bytes,
            );
        let first = files.read_observed(ROOT);
        let warm = files.read_observed(ROOT);
        let other = files.read_observed(OTHER);
        let again = files.read_observed(ROOT);
        let first_bytes = first.outcome().map_err(Clone::clone)?;
        assert!(
            ptr::eq(first_bytes, warm.outcome().map_err(Clone::clone)?),
            "each single Entry fits either budget"
        );
        assert_eq!(
            other.outcome(),
            Ok(XML),
            "a cache miss still reads the original file"
        );
        assert!(
            !ptr::eq(first_bytes, again.outcome().map_err(Clone::clone)?),
            "another Entry evicts the first under the active budget"
        );
        assert!(
            first.source().is_same_admission(again.source()),
            "eviction changes storage, not source identity"
        );
    }
    Ok(())
}

#[test]
fn cached_xml_never_bypasses_key_changes_or_verified_reads() -> TestResult {
    let corrupt = entry(OTHER, XML).with_crc32c(0);
    let (_directory, archive) = archive([entry(ROOT, XML).encrypted(), corrupt])?;
    archive.set_key(p4kx_testkit::test_key());
    let files = ArchiveFiles::new(&archive)
        .with_xml_read_cache(NonZeroUsize::new(4).ok_or("fixture entry limit")?, BUDGET);
    let first = files.read_observed(ROOT);
    assert_eq!(first.outcome(), Ok(XML), "initial usable key");
    archive.clear_key();
    let denied = files.read_observed(ROOT);
    assert!(
        matches!(denied.outcome(), Err(LoadoutFileError::Unreadable { .. })),
        "removed key is observed despite a prior cache hit"
    );
    assert!(
        first.source().is_same_admission(denied.source()),
        "key failure keeps the physical Entry origin"
    );
    archive.set_key(p4kx_testkit::test_key());
    let restored = files.read_observed(ROOT);
    assert_eq!(
        restored.outcome(),
        Ok(XML),
        "key failure does not poison later reads"
    );
    assert!(
        !ptr::eq(
            first.outcome().map_err(Clone::clone)?,
            restored.outcome().map_err(Clone::clone)?
        ),
        "reopening with another key generation reads original bytes again"
    );
    let unchecked = files.read_observed(OTHER);
    assert_eq!(
        unchecked.outcome(),
        Ok(XML),
        "unverified read establishes a retained value"
    );
    let verified = files.with_verified_entries().read_observed(OTHER);
    assert!(
        matches!(verified.outcome(), Err(LoadoutFileError::Unreadable { .. })),
        "a cached unchecked payload cannot satisfy verification"
    );
    assert!(
        unchecked.source().is_same_admission(verified.source()),
        "verification applies to the same physical Entry"
    );
    Ok(())
}

#[test]
fn cached_xml_preserves_each_callers_file_projection_and_include_evidence() -> TestResult {
    let root =
        br#"<Loadout><Loadouts><Include loadout="synthetic/other.xml"/></Loadouts></Loadout>"#;
    let child = br#"<Loadout><Items><Item portName="mount"/></Items></Loadout>"#;
    let (_directory, archive) = archive([entry(ROOT, root), entry(OTHER, child)])?;
    let files = ArchiveFiles::new(&archive)
        .with_verified_entries()
        .with_xml_read_cache(NonZeroUsize::new(2).ok_or("fixture entry limit")?, BUDGET);
    let first = observe_includes(&files)?;
    let second = observe_includes(&files)?;
    assert!(
        first.root.source().is_same_admission(second.root.source()),
        "both callers inspect the same original root Entry"
    );
    assert!(
        first
            .child
            .source()
            .is_same_admission(second.child.source()),
        "both callers inspect the same original included Entry"
    );
    assert!(
        !first.root.is_same_operation(&second.root),
        "the second root has its own observed operation"
    );
    assert!(
        !first.child.is_same_operation(&second.child),
        "the second include has its own observed operation"
    );
    Ok(())
}

struct Includes {
    root: LoadoutFileObservation,
    child: LoadoutFileObservation,
}

enum Event {
    Read(LoadoutFileObservation),
    Projection(LoadoutFileObservation),
    Dependency {
        parent: LoadoutFileObservation,
        child: LoadoutFileObservation,
    },
    Reused,
}

#[derive(Default)]
struct Events(Mutex<Vec<Event>>);

impl LoadoutReadObserver for Events {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let event = match event {
            LoadoutReadEvent::FileRead { observation, .. } => Event::Read(observation.clone()),
            LoadoutReadEvent::FileProjection { observation, .. } => {
                Event::Projection(observation.clone())
            }
            LoadoutReadEvent::FileDependency {
                parent,
                observation,
                ..
            } => Event::Dependency {
                parent: parent.clone(),
                child: observation.clone(),
            },
            LoadoutReadEvent::FileReused { .. } => Event::Reused,
            _ => return ControlFlow::Continue(()),
        };
        let Ok(mut events) = self.0.lock() else {
            return ControlFlow::Break(());
        };
        events.push(event);
        ControlFlow::Continue(())
    }
}

fn observe_includes(files: &ArchiveFiles<'_>) -> TestResult<Includes> {
    let events = Arc::new(Events::default());
    let observer: Arc<dyn LoadoutReadObserver> = events.clone();
    let observation = Observation::new(observer);
    let path = LoadoutPath::unbound(ROOT);
    let caller = LoadoutRequest::Xml { path: path.clone() };
    let result = xml::load_observed(
        files,
        &path,
        &caller,
        Some(&observation),
        None,
        &mut FileTrace::default(),
    )?;
    let [port] = result.entries.as_ref() else {
        return Err("resolve exactly one port from the included file".into());
    };
    assert_eq!(
        port.port, "mount",
        "original included port survives the cache hit"
    );
    assert!(
        result.issues.is_empty(),
        "both physical inputs were fully interpreted"
    );
    let events = std::mem::take(&mut *events.0.lock().map_err(|_| "fixture observation lock")?);
    inspect_includes(&events)
}

fn inspect_includes(events: &[Event]) -> TestResult<Includes> {
    let [
        Event::Read(root),
        Event::Projection(root_projection),
        Event::Read(child),
        Event::Projection(child_projection),
        Event::Dependency {
            parent,
            child: included,
        },
    ] = events
    else {
        return Err(
            "each caller must observe both file reads, projections and the include edge".into(),
        );
    };
    assert!(
        root.is_same_operation(root_projection) && root.is_same_operation(parent),
        "root read, parse and include edge bind the same operation"
    );
    assert!(
        child.is_same_operation(child_projection) && child.is_same_operation(included),
        "included read, parse and include edge bind the same operation"
    );
    assert_eq!(
        root.projection_outcome(),
        Some(Ok(())),
        "root parse is recorded"
    );
    assert_eq!(
        child.projection_outcome(),
        Some(Ok(())),
        "included parse is recorded"
    );
    Ok(Includes {
        root: root.clone(),
        child: child.clone(),
    })
}

fn entry(path: &str, bytes: &[u8]) -> TestEntry {
    TestEntry::stored(format!("Data/{path}"), bytes)
}

fn archive(
    entries: impl IntoIterator<Item = TestEntry>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let mut builder = ArchiveBuilder::new();
    for entry in entries {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    Ok((directory, Archive::open(path)?))
}
