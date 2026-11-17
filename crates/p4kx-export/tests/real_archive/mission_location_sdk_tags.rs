// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_export::catalog_queries::{
    MissionLocationTag, MissionLocationTagTerm, MissionLocationTemplate,
    MissionLocationTemplateSnapshot,
};
use p4kx_extraction::{
    DataForgeEntry, DataForgeOrigin, FieldOrigin, InputScope, RecordTree, SourceEncryption,
    SourceInput, SourceLocation, SourceScope, SourceUseDescription, SourceUseRef, SourceUses,
    SourceValueState, SourcedValue, StructValue, TagTree, Value as Original, ValueError,
};

use super::{Counts, TestResult};
use crate::real_archive_auto_spawns::original::Locations;

pub(super) fn compare(
    entry: &DataForgeEntry,
    tree: &RecordTree<'_>,
    row: &MissionLocationTemplate<'_>,
    tags: &TagTree,
    snapshot: &MissionLocationTemplateSnapshot<'_>,
    counts: &mut Counts,
) -> TestResult {
    let Some(Original::Struct(data)) = tree.field("locationData") else {
        return Err("original location data is not a Struct".into());
    };
    let disabled = match data.field("disabled") {
        Some(Original::Bool(value)) => Some(*value),
        _ => None,
    };
    if row.disabled() != disabled {
        return Err("SDK disabled value differs from source".into());
    }
    let evidence = Evidence::read(entry, tree, snapshot)?;
    for (name, cell) in [
        ("generalTags", row.tags()),
        ("aiSpawnTags", row.ai_spawn_tags()),
    ] {
        list(
            nested(data.field(name), "tags"),
            cell,
            tags,
            &evidence,
            counts,
        )?;
    }
    for (name, cell) in [
        ("producesTags", row.produces_tags()),
        ("consumesTags", row.consumes_tags()),
    ] {
        term(data.field(name), cell, tags, &evidence, counts)?;
    }
    Ok(())
}

fn term<'source>(
    raw: Option<&Original<'source>>,
    cell: &SourcedValue<MissionLocationTagTerm>,
    tags: &TagTree,
    evidence: &Evidence<'_, 'source>,
    counts: &mut Counts,
) -> TestResult {
    evidence.closed(cell.origin())?;
    if let Some(value) = cell.value() {
        list(
            nested(raw, "positiveTags"),
            &value.positive_tags,
            tags,
            evidence,
            counts,
        )?;
        list(
            nested(raw, "negativeTags"),
            &value.negative_tags,
            tags,
            evidence,
            counts,
        )?;
    } else if raw.is_some_and(|raw| structure(Some(raw)).is_some())
        || cell.state() != absent_state(raw)
    {
        return Err("SDK tag term state differs from its original shape".into());
    }
    Ok(())
}

const fn absent_state(original: Option<&Original<'_>>) -> SourceValueState {
    match original {
        None => SourceValueState::Absent,
        Some(Original::Null) => SourceValueState::Null,
        Some(Original::Error(_)) => SourceValueState::Unreadable,
        _ => SourceValueState::Unsupported,
    }
}

fn structure<'tree, 'source>(
    value: Option<&'tree Original<'source>>,
) -> Option<&'tree StructValue<'source>> {
    match value? {
        Original::Struct(data) => Some(data),
        Original::Array(values) => values.iter().find_map(|value| structure(Some(value))),
        _ => None,
    }
}

fn nested<'tree, 'source>(
    value: Option<&'tree Original<'source>>,
    name: &str,
) -> Option<&'tree Original<'source>> {
    structure(value).and_then(|data| data.field(name))
}

struct Evidence<'read, 'source> {
    uses: BTreeMap<SourceUseRef, &'read SourceUseDescription>,
    scope: SourceScope,
    owner: u32,
    locations: Locations<'source>,
    encrypted: Option<bool>,
}

impl<'read, 'source> Evidence<'read, 'source> {
    fn read(
        entry: &DataForgeEntry,
        tree: &RecordTree<'source>,
        snapshot: &'read MissionLocationTemplateSnapshot<'_>,
    ) -> TestResult<Self> {
        let DataForgeOrigin::Archive(original) = entry.origin() else {
            return Err("source DataForge is not an Archive Entry".into());
        };
        let inventory = SourceUses::new();
        let scope = inventory.observe(SourceInput::Archive(original), InputScope::Payload)?;
        let uses = snapshot
            .source_inventory
            .iter_uses()
            .map(|used| (used.id().clone(), used))
            .collect();
        Ok(Self {
            encrypted: match entry.origin().encryption() {
                SourceEncryption::Encrypted => Some(true),
                SourceEncryption::Unencrypted => Some(false),
                _ => None,
            },
            uses,
            scope,
            owner: tree.record_index(),
            locations: Locations::read(tree.root()),
        })
    }

    fn closed(&self, origin: &FieldOrigin) -> TestResult {
        if origin.source_uses_refs().is_empty() {
            return Err("selected field has no source evidence".into());
        }
        for reference in origin.source_uses_refs() {
            let used = self
                .uses
                .get(reference)
                .ok_or("dangling SDK source-use reference")?;
            if used.source() != self.scope.source_ref() || used.scope() != InputScope::Payload {
                return Err("tag source use addresses a foreign physical input".into());
            }
        }
        Ok(())
    }

    fn operand(
        &self,
        original: &Original<'source>,
        origin: &FieldOrigin,
        field: &str,
    ) -> TestResult {
        self.closed(origin)?;
        if origin.contains_encrypted_source() != self.encrypted {
            return Err("tag encryption marker differs from original DataForge".into());
        }
        let ordinal = u64::try_from(self.locations.ordinal(original)?)?;
        if origin.source_uses_refs().iter().any(|reference| {
            self.uses.get(reference).is_some_and(|used| {
                matches!(
                    used.location(),
                    SourceLocation::Subresource { namespace, indices, .. }
                        if namespace == "dataforgeValue"
                            && indices.first() == Some(&u64::from(self.owner))
                            && indices.as_slice() != [u64::from(self.owner), ordinal]
                )
            })
        }) {
            return Err("tag path carries evidence of a different original operand".into());
        }
        if !origin.source_uses_refs().iter().any(|reference| {
            self.uses.get(reference).is_some_and(|used| {
                matches!(
                    used.location(),
                    SourceLocation::Subresource { namespace, indices, field: actual }
                        if namespace == "dataforgeValue"
                            && indices == &[u64::from(self.owner), ordinal] && actual == field
                )
            })
        }) {
            return Err("SDK tag operand lost its exact authored slot".into());
        }
        Ok(())
    }
}

fn list<'source>(
    original: Option<&Original<'source>>,
    projected: &SourcedValue<Vec<MissionLocationTag>>,
    tags: &TagTree,
    evidence: &Evidence<'_, 'source>,
    counts: &mut Counts,
) -> TestResult {
    evidence.closed(projected.origin())?;
    let Some(Original::Array(raw)) = original else {
        if projected.value().is_some() || projected.state() != absent_state(original) {
            return Err("SDK tag list is present without an original Array".into());
        }
        return Ok(());
    };
    let values = projected
        .value()
        .ok_or("authored tag array absent from SDK")?;
    if raw.len() != values.len() {
        return Err("SDK tag array lost an authored operand slot".into());
    }
    counts.lists += 1;
    for (original, value) in raw.iter().zip(values) {
        operand(original, value, tags, evidence, counts)?;
        counts.operands += 1;
    }
    Ok(())
}

fn operand<'source>(
    original: &Original<'source>,
    value: &MissionLocationTag,
    tags: &TagTree,
    evidence: &Evidence<'_, 'source>,
    counts: &mut Counts,
) -> TestResult {
    let path = value.path().map_err(|_| "SDK tag path evidence")?;
    let field = match original {
        Original::Reference(link) => {
            let expected = tags.get(link.guid()).map_or_else(
                || link.guid().to_string(),
                |tag| tag.path_view().to_string(),
            );
            if path.value().map(ToString::to_string).as_deref() != Some(expected.as_str()) {
                return Err("SDK path differs from original Tag selection".into());
            }
            counts.resolved += 1;
            "$value"
        }
        Original::Null => {
            state(path.state(), SourceValueState::Null)?;
            counts.null += 1;
            "$value"
        }
        Original::Error(ValueError::DanglingReference { guid }) => {
            state(path.state(), SourceValueState::Absent)?;
            if value.dangling_guid().map(|(actual, _)| actual) != Some(*guid) {
                return Err("SDK dangling Tag differs from original target".into());
            }
            counts.dangling += 1;
            "$error.guid"
        }
        Original::Error(_) => {
            state(path.state(), SourceValueState::Unreadable)?;
            "$value"
        }
        _ => {
            state(path.state(), SourceValueState::Unsupported)?;
            "$value"
        }
    };
    evidence.operand(original, path.origin(), field)
}

fn state(actual: SourceValueState, expected: SourceValueState) -> TestResult {
    if actual != expected {
        return Err("SDK tag operand state differs from its original source".into());
    }
    Ok(())
}
