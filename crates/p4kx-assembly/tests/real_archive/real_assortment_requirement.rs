// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Census of the `Requirement` of the loadout assortments and their groups, and of the ones the
//! Assembly cannot decide.

#![expect(clippy::print_stdout, reason = "the census is the output")]

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;

use p4kx_assembly::{ArchiveFiles, ENTITY_TYPE, LoadoutError, LoadoutResolver};
use p4kx_dataforge::{DataForge, StructValue, Value};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const ASSORTMENT: &str = "SLoadoutAssortment";
const TAGS: &str = "SLoadoutRequirementTags";

/// What the Records hold: the groups and the Requirements of both levels.
#[derive(Default)]
struct Counts {
    assortments: usize,
    assortments_required: usize,
    groups: usize,
    groups_required: usize,
    unrestricted: usize,
    by_type: BTreeMap<String, usize>,
}

impl Counts {
    fn require(&mut self, level: &str, requirement: &StructValue<'_>) {
        *self
            .by_type
            .entry(format!("{level} {}", requirement.type_name()))
            .or_default() += 1;
    }

    fn group(&mut self, group: &StructValue<'_>) {
        self.groups += 1;
        if let Some(Value::Struct(requirement)) = group.field("Requirement") {
            self.groups_required += 1;
            self.unrestricted += usize::from(names_no_tag(requirement));
            self.require("group", requirement);
        }
    }
}

/// Returns whether the Requirement is a tag Requirement that names no tag.
fn names_no_tag(requirement: &StructValue<'_>) -> bool {
    requirement.type_name() == TAGS
        && ["RequiredTags", "BannedTags"].iter().all(
            |name| !matches!(requirement.field(name), Some(Value::Array(tags)) if !tags.is_empty()),
        )
}

fn count_requirements(forge: &DataForge<'_>) -> Counts {
    let mut counts = Counts::default();
    for record in forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == ASSORTMENT))
    {
        let Ok(tree) = record.tree() else { continue };
        counts.assortments += 1;
        if let Some(Value::Struct(requirement)) = tree.field("Requirement") {
            counts.assortments_required += 1;
            counts.require("assortment", requirement);
        }
        if let Some(Value::Array(items)) = tree.field("GroupedLoadouts") {
            for item in items {
                if let Value::Struct(group) = item {
                    counts.group(group);
                }
            }
        }
    }
    counts
}

/// Returns the number of assortments and groups that Entities list as undecided, counted once per
/// assortment GUID and group, and the number of Entities that list one.
fn undecided_of_entities(forge: &DataForge<'_>, files: &ArchiveFiles<'_>) -> (usize, usize) {
    let mut resolver = LoadoutResolver::new(forge)
        .with_files(files)
        .stopping_at_docked_vehicles();
    let mut undecided: BTreeSet<(String, Option<String>)> = BTreeSet::new();
    let mut entities = 0_usize;
    for record in forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == ENTITY_TYPE))
    {
        let Ok(node) = resolver.resolve(&record) else {
            continue;
        };
        let mut listed = false;
        for note in node.notes() {
            let LoadoutError::AssortmentRequirement {
                guid,
                assortment,
                groups,
            } = note
            else {
                continue;
            };
            let guid = guid.to_string();
            if *assortment {
                undecided.insert((guid.clone(), None));
            }
            undecided.extend(
                groups
                    .iter()
                    .map(|group| (guid.clone(), Some(group.clone()))),
            );
            listed = true;
        }
        assert!(
            node.issues().iter().all(|issue| !issue.is_note()),
            "a note is never an issue"
        );
        entities += usize::from(listed);
    }
    (undecided.len(), entities)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_requirements_of_the_assortments_are_counted_and_the_undecided_ones_noted() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, real_archive_key().ok())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;

    let counts = count_requirements(&forge);
    println!(
        "{} assortments, {} with a Requirement; {} groups, {} with a Requirement",
        counts.assortments, counts.assortments_required, counts.groups, counts.groups_required
    );
    for (kind, count) in &counts.by_type {
        println!("  {kind}: {count}");
    }
    println!(
        "{} group Requirements name no tag and restrict nothing",
        counts.unrestricted
    );

    let files = ArchiveFiles::new(&archive);
    let (undecided, entities) = undecided_of_entities(&forge, &files);
    println!(
        "{undecided} assortments or groups reached by an Entity depend on spawn tags; {entities} \
         Entities note one"
    );

    let tag_groups = counts.by_type.get("group SLoadoutRequirementTags").copied();
    assert!(counts.assortments > 0, "the Archive holds assortments");
    assert!(counts.groups_required <= counts.groups);
    assert!(
        counts.unrestricted <= tag_groups.unwrap_or(0),
        "only a tag Requirement can name no tag"
    );
    assert!(
        undecided <= counts.assortments_required + counts.groups_required,
        "only a Requirement that exists can be undecided"
    );
    assert!(
        undecided == 0 || entities > 0,
        "an undecided Requirement is noted by an Entity"
    );
    Ok(())
}
