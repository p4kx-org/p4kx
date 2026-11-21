// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_extraction::{ArchiveWatch, DataForge, Guid, Record, RecordTree, TreeBuilder};
use p4kx_extraction::{StructValue, Value as Original, ValueError};
use serde_json::{Value, json};

use super::{TestResult, rows};
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_verification::print_line;

#[derive(Clone, Copy)]
pub(super) enum Family {
    Table,
    Archetype,
}

impl Family {
    const fn kind(self) -> Kind {
        match self {
            Self::Table => Kind::LegacyLootTables,
            Self::Archetype => Kind::LegacyLootArchetypes,
        }
    }

    pub(super) const fn label(self) -> &'static str {
        self.kind().name()
    }

    pub(super) const fn pointer(self) -> &'static str {
        match self {
            Self::Table => "/legacyLootTables",
            Self::Archetype => "/legacyLootArchetypes",
        }
    }

    const fn reference(self) -> (&'static str, &'static str) {
        match self {
            Self::Table => ("archetype", "archetypeGuid"),
            Self::Archetype => ("tag", "tagGuid"),
        }
    }

    fn entries<'tree, 'source>(
        self,
        root: &'tree StructValue<'source>,
    ) -> Option<&'tree Original<'source>> {
        match self {
            Self::Table => root.field("lootArchetypes"),
            Self::Archetype => match root.field("primaryOrGroup") {
                Some(Original::Struct(group)) => group.field("entries"),
                other => other,
            },
        }
    }
}

pub(super) struct Sample<'source> {
    pub(super) tree: RecordTree<'source>,
    weight: usize,
    pub(super) dangling: Option<usize>,
}

#[derive(Debug, Default)]
struct Survey {
    admitted: usize,
    unreadable_headers: usize,
    unreadable_bodies: usize,
    absent_arrays: usize,
    null_arrays: usize,
    unreadable_arrays: usize,
    malformed_arrays: usize,
    empty_arrays: usize,
    unreadable_slots_or_operands: usize,
    malformed_slots: usize,
    null_slots: usize,
    no_finite_single: usize,
    direct_dangling: usize,
}

pub(super) fn select<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    family: Family,
) -> TestResult<Sample<'source>> {
    let kind = family.kind();
    let rule = kind.rule().ok_or("the canonical Legacy Loot rule")?;
    let mut trees = TreeBuilder::new();
    let mut survey = Survey::default();
    let mut selected = None;
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during source survey")?;
        let path = record.record_path();
        let source_type = record.type_name();
        if path.as_ref().is_ok_and(|path| !rule.matches_path(path))
            || source_type
                .as_ref()
                .is_ok_and(|name| *name != rule.type_name)
        {
            continue;
        }
        if !admitted(&record, kind, path.ok(), &mut survey, source_type.is_ok()) {
            continue;
        }
        let Ok(tree) = trees.build(&record) else {
            survey.unreadable_bodies += 1;
            continue;
        };
        if let Some((weight, dangling)) = survey.inspect(family, &tree)
            && (selected.is_none() || dangling.is_some())
        {
            selected = Some(Sample {
                tree,
                weight,
                dangling,
            });
            if dangling.is_some() {
                break;
            }
        }
    }
    print_line(&format!(
        "Legacy Loot {} inspected direct weighted arrays: {survey:?}",
        family.label()
    ));
    if let Some(sample) = selected {
        return Ok(sample);
    }
    let reason = if survey.admitted == 0 && survey.unreadable_headers == 0 {
        "no original owner admitted by the exact path and type"
    } else {
        "no readable admitted owner with a finite Single weight"
    };
    Err(format!("{}: {reason}", family.label()).into())
}

fn admitted(
    record: &Record<'_>,
    kind: Kind,
    path: Option<&str>,
    survey: &mut Survey,
    type_known: bool,
) -> bool {
    let (Some(path), Ok(name), true) = (path, record.name(), type_known) else {
        survey.unreadable_headers += 1;
        return false;
    };
    if p4kx_export::is_kind_leftover(kind, path, name) {
        return false;
    }
    survey.admitted += 1;
    true
}

impl Survey {
    fn inspect(&mut self, family: Family, tree: &RecordTree<'_>) -> Option<(usize, Option<usize>)> {
        let entries = match family.entries(tree.root()) {
            Some(Original::Array(entries)) => entries,
            None => {
                self.absent_arrays += 1;
                return None;
            }
            Some(Original::Null) => {
                self.null_arrays += 1;
                return None;
            }
            Some(Original::Error(_)) => {
                self.unreadable_arrays += 1;
                return None;
            }
            Some(_) => {
                self.malformed_arrays += 1;
                return None;
            }
        };
        if entries.is_empty() {
            self.empty_arrays += 1;
            return None;
        }
        let (source_reference, _) = family.reference();
        let mut weight = None;
        let mut dangling = None;
        for (ordinal, entry) in entries.iter().enumerate() {
            let Original::Struct(entry) = entry else {
                match entry {
                    Original::Error(_) => self.unreadable_slots_or_operands += 1,
                    Original::Null => self.null_slots += 1,
                    _ => self.malformed_slots += 1,
                }
                continue;
            };
            match entry.field("weight") {
                Some(Original::Single(value)) if value.is_finite() => {
                    weight.get_or_insert(ordinal);
                }
                Some(Original::Error(_)) => self.unreadable_slots_or_operands += 1,
                _ => {}
            }
            match entry.field(source_reference) {
                Some(Original::Error(ValueError::DanglingReference { .. })) => {
                    self.direct_dangling += 1;
                    dangling.get_or_insert(ordinal);
                }
                Some(Original::Error(_)) => self.unreadable_slots_or_operands += 1,
                _ => {}
            }
        }
        if weight.is_none() {
            self.no_finite_single += 1;
        }
        weight.map(|position| (position, dangling))
    }
}

pub(super) fn check(
    family: Family,
    sample: &Sample<'_>,
    row: &Value,
    origins: &Origins<'_>,
) -> TestResult {
    let tree = &sample.tree;
    let expected_guid = tree.guid().to_string();
    if row.pointer("/guid/value").and_then(Value::as_str) != Some(expected_guid.as_str())
        || row.pointer("/recordPath/value").and_then(Value::as_str) != Some(tree.record_path())
    {
        return Err("the result differs from the physically selected original Record".into());
    }
    let Some(Original::Array(original)) = family.entries(tree.root()) else {
        return Err("the original weighted array is unavailable".into());
    };
    let rendered = rows(row, "/entries/value")?;
    if rendered.len() != original.len() {
        return Err("the exact authored weighted array length differs".into());
    }
    let locations = Locations::read(tree.root());
    let raw = operand(original, sample.weight, "weight")?;
    let typed = at(
        rendered
            .get(sample.weight)
            .ok_or("the original weight slot")?,
        "/value/weight",
    )?;
    check_weight(
        typed,
        raw,
        origins,
        tree.record_index(),
        locations.ordinal(raw)?,
    )?;
    if let Some(slot) = sample.dangling {
        let (source, wire) = family.reference();
        let raw = operand(original, slot, source)?;
        let Original::Error(ValueError::DanglingReference { guid }) = *raw else {
            return Err("the nominated original operand did not have a dangling GUID".into());
        };
        let typed = rendered
            .get(slot)
            .and_then(|entry| entry.pointer(&format!("/value/{wire}")))
            .ok_or("the typed original dangling Reference")?;
        check_dangling(
            row,
            typed,
            origins,
            tree.record_index(),
            locations.ordinal(raw)?,
            guid,
        )?;
    }
    Ok(())
}

fn check_weight(
    typed: &Value,
    raw: &Original<'_>,
    origins: &Origins<'_>,
    record: u32,
    ordinal: usize,
) -> TestResult {
    let Original::Single(weight) = *raw else {
        return Err("the selected original weight is not a Single".into());
    };
    let finite: Value = serde_json::from_str(&serde_json::to_string(&weight)?)?;
    let expected =
        json!({"type":"f32","rawBits":format!("{:08x}", weight.to_bits()),"finite":finite});
    if at(typed, "/value")? != &expected {
        return Err("the weighted occurrence differs from its exact original Single".into());
    }
    origins.occurrence(typed, record, ordinal, "$value")
}

fn check_dangling(
    row: &Value,
    typed: &Value,
    origins: &Origins<'_>,
    record: u32,
    ordinal: usize,
    guid: Guid,
) -> TestResult {
    let expected = json!(guid.to_string());
    let node = rows(row, "/values")?
        .get(ordinal)
        .ok_or("the exact original reference arena occurrence")?;
    let raw_guid = at(node, "/originalError/guid")?;
    if at(typed, "/state")? != "present"
        || at(typed, "/sourceSetComplete")? != false
        || at(typed, "/value")? != &expected
        || at(node, "/originalError/type")? != "danglingReference"
        || at(raw_guid, "/value")? != &expected
    {
        return Err("the typed GUID or the retained original lookup failure differs".into());
    }
    origins.occurrence(raw_guid, record, ordinal, "$error.guid")?;
    let resolved = origins.with_resolved_contributors(typed)?;
    let location = json!({
        "type": "subresource",
        "namespace": "dataforgeValue",
        "indices": [record.to_string(), ordinal.to_string()],
        "field": "$error.guid"
    });
    if !rows(&resolved, "/sourceUsesRefs")?
        .iter()
        .any(|used| used["role"] == "payload" && used["location"] == location)
    {
        return Err("the typed GUID omits its actual original error-parameter contributor".into());
    }
    Ok(())
}

fn operand<'tree, 'source>(
    entries: &'tree [Original<'source>],
    slot: usize,
    field: &str,
) -> TestResult<&'tree Original<'source>> {
    let Some(Original::Struct(entry)) = entries.get(slot) else {
        return Err("the selected original weighted array slot is absent".into());
    };
    entry
        .field(field)
        .ok_or_else(|| "the selected original operand is absent".into())
}

fn at<'value>(value: &'value Value, path: &str) -> TestResult<&'value Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing Legacy Loot wire member {path}").into())
}
