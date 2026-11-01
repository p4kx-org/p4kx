// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The two semantic questions of the Loadout of a real Archive: how often a Loadout entry names its
//! Entity by Reference and by class name with the two disagreeing (the Reference wins), and how
//! many ports of ships and vehicles are filled only by the default item of their definition.

#![expect(clippy::print_stdout, reason = "the counts are the measurement")]

use std::error::Error;

use p4kx_assembly::{
    ArchiveFiles, Attachment, ENTITY_TYPE, EntityCatalogue, EntityKind, LoadoutResolver, PortSource,
};
use p4kx_dataforge::{DataForge, StructValue, Value, ValueError};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const MAX_EXAMPLES: usize = 5;
const RECORD_NAME_PREFIX: &str = "EntityClassDefinition.";

/// The fewest ships and vehicles a walk of the fleet resolves; LIVE has 1,144.
const MIN_FLEET: usize = 1000;

/// The Records and names LIVE fails to read in the census; more would make its counts untrusted.
const EXPECTED_UNREADABLE: usize = 0;

/// The ships and vehicles LIVE fails to find or resolve in the census.
const EXPECTED_UNRESOLVED: usize = 0;

#[derive(Default)]
struct Entries {
    both: usize,
    disagreeing: usize,
    /// Entries whose Reference points to no Record.
    dangling: usize,
    /// Entries whose referenced Record has no readable name.
    unreadable: usize,
    examples: Vec<String>,
}

impl Entries {
    /// Counts the entries among `value` that carry a Reference and a class name.
    fn count(&mut self, forge: &DataForge<'_>, value: &Value<'_>) {
        match value {
            Value::Struct(structure) => {
                self.count_entry(forge, structure);
                for field in structure.fields() {
                    self.count(forge, field.value());
                }
            }
            Value::Array(items) => {
                for item in items {
                    self.count(forge, item);
                }
            }
            _ => {}
        }
    }

    fn count_entry(&mut self, forge: &DataForge<'_>, entry: &StructValue<'_>) {
        let Some(Value::String(class_name)) = entry.field("entityClassName") else {
            return;
        };
        if class_name.is_empty() {
            return;
        }
        let guid = match entry.field("entityClassReference") {
            Some(Value::Reference(link)) => Some(link.guid()),
            Some(Value::Error(ValueError::DanglingReference { .. })) => None,
            _ => return,
        };
        self.both += 1;
        let Some(record) = guid.and_then(|guid| forge.record_by_guid(guid)) else {
            self.dangling += 1;
            return;
        };
        let Ok(name) = record.name() else {
            self.unreadable += 1;
            return;
        };
        let referenced = name.strip_prefix(RECORD_NAME_PREFIX).unwrap_or(name);
        if !referenced.eq_ignore_ascii_case(class_name) {
            self.disagreeing += 1;
            if self.examples.len() < MAX_EXAMPLES {
                self.examples
                    .push(format!("{class_name} against {referenced}"));
            }
        }
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_reference_and_a_class_name_that_disagree_are_counted() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut entries = Entries::default();
    let mut unreadable_records = 0_usize;
    for record in forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == ENTITY_TYPE))
    {
        let Ok(tree) = record.tree() else {
            unreadable_records += 1;
            continue;
        };
        if let Some(components) = tree.field("Components") {
            entries.count(&forge, components);
        }
    }
    println!(
        "entries with a Reference and a class name: {}, disagreeing: {}, dangling: {}, \
         unreadable names: {}, unreadable Records: {unreadable_records}, e.g. {:?}",
        entries.both, entries.disagreeing, entries.dangling, entries.unreadable, entries.examples
    );
    assert!(entries.both > 0, "some entries carry both");
    assert!(
        entries.disagreeing > 0,
        "data-dependent: LIVE has entries whose Reference and class name differ; a build \
         without any makes the order of the two moot and this check obsolete"
    );
    assert!(entries.disagreeing <= entries.both);
    assert_eq!(
        unreadable_records + entries.unreadable,
        EXPECTED_UNREADABLE,
        "unreadable Records plus names"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn ports_filled_only_by_a_default_item_are_counted() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let catalogue = EntityCatalogue::build(&forge, &|| false).ok_or("catalogue stopped")?;
    let (mut vehicles, mut unresolved) = (0_usize, 0_usize);
    let (mut ports, mut by_default, mut with_default) = (0_usize, 0_usize, 0_usize);
    for summary in catalogue.entries() {
        if !matches!(summary.kind(), EntityKind::Ship | EntityKind::Vehicle) {
            continue;
        }
        let Ok(record) = resolver.entities().find(summary.class_name()) else {
            unresolved += 1;
            continue;
        };
        let Ok(tree) = resolver.resolve(&record) else {
            unresolved += 1;
            continue;
        };
        vehicles += 1;
        let mut has_default = false;
        for (_, port) in tree.walk() {
            if matches!(port.attachment(), Attachment::Entity(_)) {
                ports += 1;
                if port.source() == PortSource::PortDefault {
                    by_default += 1;
                    has_default = true;
                }
            }
        }
        with_default += usize::from(has_default);
    }
    println!(
        "{vehicles} ships and vehicles, {ports} ports holding an Entity, {by_default} of them \
         filled only by a default item, in {with_default} of the vehicles, {unresolved} not \
         resolved"
    );
    assert!(
        vehicles >= MIN_FLEET,
        "the whole fleet was walked: {vehicles}"
    );
    assert_eq!(
        unresolved, EXPECTED_UNRESOLVED,
        "ships and vehicles not resolved"
    );
    assert!(by_default > 0, "default items fill ports no entry names");
    Ok(())
}
