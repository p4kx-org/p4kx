// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, ProvenanceMode, RecordTree, Step, Value,
};
use p4kx_testkit::{real_archive_key, watchdog};

use crate::Kind;
use crate::catalog_queries::loot::LootDefinitionKind;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
};
use crate::entity_selection::{self, Selected};

#[path = "installed_loot_delta_fixture.rs"]
mod fixture;
#[path = "installed_loot_delta_optional.rs"]
mod optional;
#[path = "installed_loot_delta_projection.rs"]
mod projection;

use fixture::{Profile, Side};
use optional::OriginalOptional;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct OriginalEntry {
    owner: u32,
    ordinal: u64,
    projected_entry: usize,
    bits: u32,
    optional: OriginalOptional,
}

#[test]
#[ignore = "real Archives: bind both original sides with P4KX_LOOT_DELTA_FIXTURE"]
fn installed_loot_weight_delta_matches_original_and_scoped_producer() -> TestResult {
    watchdog::start();
    let profile = Profile::read()?;
    let before = run(&profile.old)?;
    let after = run(&profile.new)?;
    if before == after {
        return Err("the selected original weights did not change between inputs".into());
    }
    Ok(())
}

fn run(side: &Side) -> TestResult<u32> {
    let key = real_archive_key().map_err(|_| "installed Archive key unavailable")?;
    let archive = Archive::open_with_key(&side.archive, Some(key))
        .map_err(|_| "audited Archive could not open")?;
    side.check_archive(&archive)?;
    let watch = ArchiveWatch::new(&archive);
    watch.check().map_err(|_| "audited Archive changed")?;
    let entry = DataForgeEntry::read(&archive).map_err(|_| "audited DataForge could not read")?;
    side.dataforge.check_entry(&archive, &entry)?;
    let forge = entry
        .open()
        .map_err(|_| "audited DataForge could not open")?;
    let record = forge
        .record(side.record.index)
        .ok_or("selected physical Loot owner is absent")?;
    let admission = entity_selection::select(&record, None)
        .map_err(|_| "selected Loot owner could not be classified")?;
    let tree = record
        .tree()
        .map_err(|_| "selected original Loot owner could not read")?;
    if admission != Some(Selected::Entity(Kind::LootTables))
        || tree.guid().to_string() != side.record.guid
        || tree.record_path() != side.record.record_path
    {
        return Err("selected physical owner does not match the audited Loot Record".into());
    }
    let original = OriginalEntry::locate(&tree, side)?;
    let producer = projection::producer(&archive, &entry, &forge, &record, &tree, side, &original)?;
    let session = CatalogQuerySession::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )
    .map_err(|_| "could not bind the scoped Loot SDK session")?
    .with_provenance_mode(ProvenanceMode::Full);
    session
        .with_loot_definitions(
            LootDefinitionKind::Table,
            CatalogRecordSelection::RecordIndex(record.index()),
            None,
            |snapshot| projection::compare(&snapshot, &producer, side, &original),
        )
        .map_err(|_| "scoped Loot SDK query could not complete")??;
    watch
        .check()
        .map_err(|_| "audited Archive changed after the proof")?;
    Ok(original.bits)
}

impl OriginalEntry {
    fn locate(tree: &RecordTree<'_>, side: &Side) -> TestResult<Self> {
        let path = [
            Step::Field("lootTable"),
            Step::Field("lootArchetypes"),
            Step::Element(side.record.entry_ordinal),
            Step::Field("weight"),
        ];
        let mut matching = 0;
        let mut original = None;
        tree.root().visit_owned_values(|visit| {
            if visit.steps() == path {
                matching += 1;
                if let Value::Single(value) = visit.value() {
                    original = Some((visit.ordinal(), value.to_bits()));
                }
            }
        });
        let (position, bits) = original.ok_or("the audited field is not an original Single")?;
        if matching != 1 || bits != side.record.weight_bits {
            return Err("the original source does not match the audited weight".into());
        }
        Ok(Self {
            owner: tree.record_index(),
            ordinal: u64::try_from(position).map_err(|_| "original occurrence not addressable")?,
            projected_entry: projected_entry(tree, side.record.entry_ordinal)?,
            bits,
            optional: OriginalOptional::read(tree, &side.record)?,
        })
    }
}

fn projected_entry(tree: &RecordTree<'_>, ordinal: u32) -> TestResult<usize> {
    let Some(Value::Struct(table)) = tree.field("lootTable") else {
        return Err("original Loot table is not a struct".into());
    };
    let Some(Value::Array(entries)) = table.field("lootArchetypes") else {
        return Err("original Loot table entries are missing".into());
    };
    let index = usize::try_from(ordinal).map_err(|_| "original entry index is not addressable")?;
    let Some((prefix, [Value::Struct(_), ..])) = entries.split_at_checked(index) else {
        return Err("original selected entry is not a struct".into());
    };
    Ok(prefix
        .iter()
        .filter(|value| matches!(value, Value::Struct(_)))
        .count())
}
