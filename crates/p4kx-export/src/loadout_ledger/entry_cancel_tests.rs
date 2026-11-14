// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{ArchiveFiles, LoadoutFileError, LoadoutFileIdentity, LoadoutFiles};
use p4kx_extraction::CancelToken;
use p4kx_testkit::TestEntry;

use super::entries::CONFIGURED;
use super::{
    Arc, Capture, DataForge, DataForgeBuilder, Entities, LoadoutResolver, ROOT, TestEntity,
    TestLoadout, TestResult,
};

struct CancellingFiles<'a> {
    files: ArchiveFiles<'a>,
    cancel: CancelToken,
    reads: AtomicUsize,
}

impl LoadoutFiles for CancellingFiles<'_> {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        let result = self.files.read(path);
        self.cancel.cancel();
        result
    }

    fn identity(&self, path: &str) -> Option<LoadoutFileIdentity<'_>> {
        self.files.identity(path)
    }
}

#[test]
fn cancelling_an_actual_file_read_stops_includes_and_propagates_the_original_cancel() -> TestResult
{
    let (_directory, archive) = super::entries::archive(vec![TestEntry::stored(format!("Data/{CONFIGURED}"),
        b"<Loadout><Loadouts><Loadout loadout=\"Scripts/Loadouts/Synthetic/next.xml\"/></Loadouts></Loadout>".to_vec())])?;
    let cancel = CancelToken::new();
    let files = CancellingFiles {
        files: ArchiveFiles::new(&archive),
        cancel: cancel.clone(),
        reads: AtomicUsize::new(0),
    };
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(ROOT, "Root", "synthetic/root.xml")
                .loadout(TestLoadout::Xml(CONFIGURED.to_owned()))
                .record(),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("source missing")?;
    let tree = record.tree()?;
    let capture = Arc::new(Capture::new(&super::source(&record, &tree)?, Some(&cancel)));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone())
            .with_files(&files);
    assert!(
        matches!(
            resolver.resolve(&record),
            Err(p4kx_assembly::LoadoutError::ObservationStopped)
        ),
        "the actual stopped observer reaches the resolver caller"
    );
    assert_eq!(
        files.reads.load(Ordering::Relaxed),
        1,
        "no include or fallback reads after cancellation"
    );
    assert!(
        matches!(capture.finish(), Err(crate::CatalogError::Cancelled)),
        "the original cancellation is not hidden by optional loadout mapping"
    );
    Ok(())
}
