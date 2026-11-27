// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write as _;

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    MissionLocationTemplateSnapshot, ProvenanceMode,
};
use p4kx_extraction::TagTree;

use super::{Archive, ArchiveWatch, DataForgeEntry, Filter, RecordTree, TestResult, TreeBuilder};
use super::{Witnesses, real_archive_key, real_archive_path};

#[path = "mission_location_sdk_tags.rs"]
mod tags;

#[derive(Default)]
struct Counts {
    records: usize,
    lists: usize,
    operands: usize,
    resolved: usize,
    dangling: usize,
    null: usize,
    recoverable_owners: usize,
}

impl Counts {
    fn report(&self) -> TestResult {
        if self.records == 0 || self.lists == 0 || self.resolved == 0 {
            return Err("selected original fixtures do not exercise SDK tag projection".into());
        }
        writeln!(
            std::io::stderr().lock(),
            "Mission location SDK source check: {} records, {} lists, {} operands, {} resolved, {} dangling, {} null, {} recoverable owners",
            self.records,
            self.lists,
            self.operands,
            self.resolved,
            self.dangling,
            self.null,
            self.recoverable_owners,
        )?;
        Ok(())
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn selected_mission_location_sdk_slots_match_original_sources() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let archive = Archive::open_with_key(
        path,
        Some(real_archive_key().map_err(|_| "configured Archive key")?),
    )
    .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    watch.check().map_err(|_| "Archive changed")?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "open location SDK session")?
    .with_provenance_mode(ProvenanceMode::Full);
    let parsed = session
        .parsed(None)
        .map_err(|_| "parse location SDK source")?;
    let forge = entry
        .open()
        .map_err(|_| "open independent original DataForge")?;
    let scope = std::env::var("P4KX_LOCATION_CONFIGURATION_FILTER")
        .ok()
        .map(|filter| Filter::parse(&filter).map_err(|_| "invalid configuration scope"))
        .transpose()?;
    let selected = Witnesses::read(&forge, &watch, scope.as_ref())?.indices()?;
    let tags = TagTree::build_with(&forge, &|| watch.poll().is_err())
        .ok_or("original Tag tree interrupted")?;
    let mut counts = Counts::default();
    for index in selected {
        watch.check().map_err(|_| "Archive changed")?;
        let record = forge.record(index).ok_or("selected physical owner")?;
        let tree = TreeBuilder::new()
            .build(&record)
            .map_err(|_| "original selected body")?;
        parsed
            .with_mission_location_templates(
                CatalogRecordSelection::RecordIndex(index),
                None,
                |snapshot| compare(&archive, &entry, &tree, &tags, &snapshot, &mut counts),
            )
            .map_err(|_| "selected location SDK projection")??;
        watch.check().map_err(|_| "Archive changed")?;
    }
    counts.report()
}

fn compare(
    archive: &Archive,
    entry: &DataForgeEntry,
    tree: &RecordTree<'_>,
    original_tags: &TagTree,
    snapshot: &MissionLocationTemplateSnapshot<'_>,
    counts: &mut Counts,
) -> TestResult {
    check_build(archive, snapshot)?;
    if !matches!(snapshot.availability, DatasetAvailability::Available)
        || !snapshot.coverage.unreadable_headers.is_empty()
        || !snapshot.coverage.unreadable_bodies.is_empty()
        || snapshot.coverage.skipped_leftovers != 0
        || !snapshot.available_languages.is_empty()
    {
        return Err("selected SDK result has unexpected coverage or locale reads".into());
    }
    counts.recoverable_owners += snapshot.coverage.incomplete_fields.len();
    let [row] = snapshot.rows.as_slice() else {
        return Err("selected physical owner must yield one SDK row".into());
    };
    if row.record_index() != tree.record_index()
        || row.guid() != tree.guid()
        || row.record_path().value().copied() != Some(tree.record_path())
    {
        return Err("SDK row identity differs from its original physical owner".into());
    }
    tags::compare(entry, tree, row, original_tags, snapshot, counts)?;
    counts.records += 1;
    Ok(())
}

fn check_build(archive: &Archive, snapshot: &MissionLocationTemplateSnapshot<'_>) -> TestResult {
    let expected = archive.build().ok_or("installed Build identity")?;
    let actual = snapshot.build.as_ref().ok_or("SDK Build identity")?;
    if [
        expected.branch(),
        expected.version(),
        expected.change_number(),
        expected.build_id(),
    ] != [
        actual.branch(),
        actual.version(),
        actual.change_number(),
        actual.build_id(),
    ] {
        return Err("SDK Build differs from installed Archive".into());
    }
    Ok(())
}
