// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Survey of the error values in the Records of the companion-app reference types on a real
//! Archive: which error each value carries and where.

use std::collections::BTreeMap;

use p4kx_extraction::{Archive, DataForge, DataForgeEntry, TreeBuilder, ValueError};
use p4kx_testkit::real_archive_path;

use super::{TestResult, report};

const TYPES: [&str; 11] = [
    "SLoadoutAssortment",
    "AIWaveCollection",
    "MissionBrokerEntry",
    "MissionLocationTemplate",
    "ResourceType",
    "StarMapObject",
    "MissionItem",
    "ContractGenerator",
    "Faction",
    "CraftingBlueprintRecord",
    "SCItemManufacturer",
];
const EXAMPLES: usize = 3;
const MAX_DEPTH: usize = 64;

#[derive(Default)]
struct Group {
    count: usize,
    examples: Vec<String>,
}

fn strip_indices(location: &str) -> String {
    let mut out = String::with_capacity(location.len());
    let mut inside = false;
    for character in location.chars() {
        match character {
            '[' => {
                inside = true;
                out.push_str("[]");
            }
            ']' => inside = false,
            other if !inside => out.push(other),
            _ => {}
        }
    }
    out
}

const fn kind(error: &ValueError) -> &'static str {
    match error {
        ValueError::InvalidString { .. } => "InvalidString",
        ValueError::UnknownEnum { .. } => "UnknownEnum",
        ValueError::UnknownStruct { .. } => "UnknownStruct",
        ValueError::InvalidBool { .. } => "InvalidBool",
        ValueError::InvalidPointer { .. } => "InvalidPointer",
        ValueError::ArrayOutOfRange { .. } => "ArrayOutOfRange",
        ValueError::OwnedTwice { .. } => "OwnedTwice",
        ValueError::InlineClassCycle { .. } => "InlineClassCycle",
        ValueError::DanglingReference { .. } => "DanglingReference",
        ValueError::OutsideRecord { .. } => "OutsideRecord",
        _ => "Other",
    }
}

fn struct_note(forge: &DataForge<'_>, struct_index: u32) -> String {
    forge.structs().nth(struct_index as usize).map_or_else(
        || "struct index out of range".to_owned(),
        |found| {
            format!(
                "struct {} ({} instances)",
                found.name().unwrap_or("?"),
                found.instance_count()
            )
        },
    )
}

fn note(forge: &DataForge<'_>, error: &ValueError) -> String {
    match error {
        ValueError::InlineClassCycle { struct_index }
        | ValueError::InvalidPointer { struct_index, .. }
        | ValueError::OwnedTwice { struct_index, .. }
        | ValueError::OutsideRecord { struct_index, .. } => struct_note(forge, *struct_index),
        ValueError::UnknownStruct { index } => format!("struct index {index}"),
        _ => String::new(),
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn error_values_of_the_reference_types_are_surveyed() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut builder = TreeBuilder::new();
    let mut groups: BTreeMap<(&str, String, &str), Group> = BTreeMap::new();
    let mut affected: BTreeMap<&str, usize> = BTreeMap::new();

    for record in forge.records() {
        let Some(type_name) = reference_type(&record) else {
            continue;
        };
        let Ok(tree) = builder.build(&record) else {
            continue;
        };
        let mut any = false;
        for (location, error) in tree.located_errors() {
            any = true;
            let text = location.to_string();
            let group = groups
                .entry((type_name, strip_indices(&text), kind(error)))
                .or_default();
            group.count += 1;
            if group.examples.len() < EXAMPLES {
                group.examples.push(format!(
                    "{} @ {text}: {error}; {}",
                    record.record_path().unwrap_or("?"),
                    note(&forge, error)
                ));
            }
        }
        if any {
            *affected.entry(type_name).or_default() += 1;
        }
    }

    print(&affected, &groups);
    Ok(())
}

fn reference_type(record: &p4kx_extraction::Record<'_>) -> Option<&'static str> {
    let mut current = record.struct_type().ok();
    let mut steps = 0;
    while let Some(found) = current
        && steps < MAX_DEPTH
    {
        steps += 1;
        if let Ok(name) = found.name()
            && let Some(hit) = TYPES.iter().find(|candidate| **candidate == name)
        {
            return Some(*hit);
        }
        current = found.parent();
    }
    None
}

fn print(affected: &BTreeMap<&str, usize>, groups: &BTreeMap<(&str, String, &str), Group>) {
    report("Records with error values per type");
    for (name, records) in affected {
        report(&format!("  {name}: {records} Records"));
    }
    report("Error values per type, field path and kind");
    for ((type_name, path, kind), group) in groups {
        report(&format!(
            "  {type_name} | {path} | {kind} | {}",
            group.count
        ));
        for example in &group.examples {
            report(&format!("      {example}"));
        }
    }
}
