// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, DataForgeEntry, StructValue, Value};

use crate::contracts_fixture as fixture;
use crate::starmap_fixture;

pub(crate) fn locations(archive: &Archive) -> Result<Vec<String>, Box<dyn Error>> {
    let loaded = DataForgeEntry::read(archive)?;
    let forge = loaded.open()?;
    let generator = forge
        .record_by_guid(starmap_fixture::guid_text(fixture::GENERATOR).parse()?)
        .ok_or("original mission generator")?
        .tree()?;
    let contract = find_contract(generator.root())?;
    let Some(Value::Array(prerequisites)) = contract.field("additionalPrerequisites") else {
        return Err("original contract prerequisites array".into());
    };
    let [Value::Struct(prerequisite)] = prerequisites.as_ref() else {
        return Err("one original locality prerequisite".into());
    };
    let Some(Value::Reference(locality)) = prerequisite.field("localityAvailable") else {
        return Err("original locality reference".into());
    };
    assert_eq!(
        locality.guid().to_string(),
        starmap_fixture::guid_text(fixture::LOC_B),
        "the actual contract selects the seven-place locality"
    );
    let tree = forge
        .record(locality.record_index())
        .ok_or("original locality target")?
        .tree()?;
    assert_eq!(
        tree.error_count(),
        0,
        "the original locality is fully readable"
    );
    let Some(Value::Array(locations)) = tree.field("availableLocations") else {
        return Err("original locality locations array".into());
    };
    locations.iter().map(location_guid).collect()
}

fn find_contract<'tree, 'source>(
    root: &'tree StructValue<'source>,
) -> Result<&'tree StructValue<'source>, Box<dyn Error>> {
    let wanted = starmap_fixture::guid_text(fixture::CONTRACT_B);
    let mut pending = vec![root];
    while let Some(structure) = pending.pop() {
        if matches!(structure.field("id"), Some(Value::Guid(guid)) if guid.to_string() == wanted) {
            return Ok(structure);
        }
        for field in structure.fields() {
            if let Value::Struct(child) = field.value() {
                pending.push(child);
            } else if let Value::Array(values) = field.value() {
                for value in values {
                    if let Value::Struct(child) = value {
                        pending.push(child);
                    }
                }
            }
        }
    }
    Err("original contract B in its actual generator tree".into())
}

fn location_guid(value: &Value<'_>) -> Result<String, Box<dyn Error>> {
    let Value::Reference(reference) = value else {
        return Err("every original locality slot is a readable Reference".into());
    };
    assert_eq!(
        reference.type_name(),
        "StarMapObject",
        "every original slot resolves to a physical starmap object"
    );
    Ok(reference.guid().to_string())
}
