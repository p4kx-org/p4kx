// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    ContractEntry, ContractSnapshot, MissionPropertyDeclaration, PropertyValue,
};
use p4kx_extraction::{RecordTree, Value};

use super::source::{self, Rows, Site};
use super::{Counts, TestResult};

pub(super) fn compare(
    tree: &RecordTree<'_>,
    sources: &Rows<'_, '_>,
    site: &Site<'_, '_>,
    row: &ContractEntry<'_>,
    snapshot: &ContractSnapshot<'_>,
    counts: &mut Counts,
) -> TestResult {
    let expected: Vec<_> = site
        .sets
        .iter()
        .flatten()
        .map(|&index| sources.sets.get(index).ok_or("original set index"))
        .collect::<Result<_, _>>()?;
    if row.base.property_set_guids.len() != expected.len() {
        return Err("SDK lost or invented an effective original set link".into());
    }
    for (guid, original) in row.base.property_set_guids.iter().zip(expected) {
        let set = snapshot
            .mission_property_sets
            .iter()
            .find(|set| &set.guid == guid)
            .ok_or("effective SDK property-set reference is not closed")?;
        if set.record_guid.get() != tree.guid()
            || set.record_path != tree.record_path()
            || set.source_type != tree.type_name()
            || set.source_location != original.location
            || std::mem::discriminant(&set.level) != std::mem::discriminant(&original.level)
            || set.properties.len() != original.properties.len()
        {
            return Err("SDK property-set link has a different original owner, level, location or slot count".into());
        }
        for (raw, projected) in original.properties.iter().zip(&set.properties) {
            declaration(raw, projected.as_ref())?;
        }
        counts.set_links += 1;
    }
    Ok(())
}

fn declaration(
    original: &Value<'_>,
    projected: Option<&MissionPropertyDeclaration<'_>>,
) -> TestResult {
    let original = source::structure(Some(original))?;
    let (original, projected) = match (original, projected) {
        (None, None) => return Ok(()),
        (Some(original), Some(projected)) => (original, projected),
        _ => return Err("SDK property declaration lost a null or owned source slot".into()),
    };
    let value = source::structure(original.field("value"))?;
    let value_type = value.map(|value| {
        value
            .type_name()
            .strip_prefix("MissionPropertyValue_")
            .unwrap_or_else(|| value.type_name())
    });
    if projected.variable.as_deref() != source::text(original, "missionVariableName")
        || projected.token.as_deref() != source::text(original, "extendedTextToken")
        || projected.value_type.as_deref() != value_type
        || projected.value.is_some() != value.is_some()
    {
        return Err("SDK property declaration differs from its original variable, token, concrete type or null state".into());
    }
    if let Some(value) = value.filter(|value| value.type_name() == "MissionPropertyValue_Boolean") {
        let Some(PropertyValue::Boolean { value: actual }) = &projected.value else {
            return Err("the original Boolean property lost its typed value".into());
        };
        if *actual != source::boolean(value, "value") {
            return Err("SDK Boolean property differs from its original value".into());
        }
    }
    Ok(())
}
