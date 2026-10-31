// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Loadouts generated from `SLoadoutAssortment` Records: the alternative of each group whose
//! Requirement is not known to fail, the first unless [`LoadoutPicks`] choose another.

use std::collections::HashSet;
use std::sync::Arc;

use p4kx_dataforge::{Guid, Record, StructValue, TreeBuilder, Value};

use crate::LoadoutError;
use crate::assortment_group::Group;
use crate::assortment_requirement::{Verdict, evaluate};
use crate::pick::LoadoutPicks;
use crate::read::{as_struct, struct_items, text};
use crate::resolver::{Entries, LoadoutResolver};
use crate::spec::{LoadoutSpec, TargetSpec, read_loadout, reference_target};

const ASSORTMENT_TYPE: &str = "SLoadoutAssortment";

/// What an assortment Record gives: its groups with every alternative, and why parts of it
/// cannot be used. The picks are applied after reading, so one read serves every pick.
#[derive(Debug, Clone)]
pub(crate) struct Assorted {
    guid: Guid,
    groups: Arc<[Group]>,
    issues: Arc<[LoadoutError]>,
}

impl Assorted {
    fn new(guid: Guid, groups: Vec<Group>, issues: Vec<LoadoutError>) -> Self {
        Self {
            guid,
            groups: groups.into(),
            issues: issues.into(),
        }
    }

    /// Returns the Loadouts to lay over each other, one per group in order, adds to `issues`
    /// what keeps part of the assortment from being used and to `seen` the lower-case names of
    /// its groups.
    fn layers(
        &self,
        picks: &LoadoutPicks,
        seen: &mut HashSet<String>,
        issues: &mut Vec<LoadoutError>,
    ) -> Vec<LoadoutSpec> {
        issues.extend(self.issues.iter().cloned());
        seen.extend(self.groups.iter().map(|g| g.name().to_ascii_lowercase()));
        self.groups
            .iter()
            .filter_map(|group| group.chosen(picks, self.guid, issues))
            .collect()
    }
}

/// Reads the assortments a generated Loadout names; `None` when it names none.
pub(crate) fn read_assortments(loadout: &StructValue<'_>) -> Option<LoadoutSpec> {
    let Some(Value::Array(items)) = loadout.field("LoadoutAssortments") else {
        return None;
    };
    let guids: Arc<[Guid]> = items
        .iter()
        .filter_map(|item| match reference_target(item) {
            TargetSpec::Reference(guid) => Some(guid),
            _ => None,
        })
        .collect();
    (!guids.is_empty()).then_some(LoadoutSpec::Assortments(guids))
}

/// The Requirements of one assortment that the Archive cannot decide.
#[derive(Default)]
struct Undecided {
    assortment: bool,
    groups: Vec<String>,
}

impl Undecided {
    /// Returns whether the assortment, or its group `group`, is used given its Requirement
    /// `field`; an undecided Requirement is used and remembered.
    fn applies(&mut self, group: Option<&str>, field: Option<&Value<'_>>) -> bool {
        match evaluate(field) {
            Verdict::Met => true,
            Verdict::Unmet => false,
            Verdict::Undecided => {
                match group {
                    None => self.assortment = true,
                    Some(name) if !self.groups.iter().any(|known| known == name) => {
                        self.groups.push(name.to_owned());
                    }
                    Some(_) => {}
                }
                true
            }
        }
    }

    fn into_note(self, guid: Guid) -> Option<LoadoutError> {
        (self.assortment || !self.groups.is_empty()).then_some(
            LoadoutError::AssortmentRequirement {
                guid,
                assortment: self.assortment,
                groups: self.groups,
            },
        )
    }
}

/// Reads one assortment Record: every alternative of every group, in the order of the groups.
///
/// The game picks one alternative per group at random; without a pick the first is the
/// deterministic default. An assortment or group whose Requirement the Archive decides as unmet
/// gives nothing; one that depends on spawn tags still gives its groups and is one
/// `assortment_requirement` note.
fn read_assorted<'f>(builder: &mut TreeBuilder<'f>, record: &Record<'f>) -> Assorted {
    let guid = record.guid();
    if !matches!(record.type_name(), Ok(ASSORTMENT_TYPE)) {
        let found = record.type_name().unwrap_or("Record of an unreadable type");
        let reason = format!("the Record is a {found}, not an assortment");
        let issue = LoadoutError::Assortment { guid, reason };
        return Assorted::new(guid, Vec::new(), vec![issue]);
    }
    let tree = match builder.build(record) {
        Ok(tree) => tree,
        Err(error) => {
            let issue = LoadoutError::UnreadableRecord {
                guid,
                reason: error.to_string(),
            };
            return Assorted::new(guid, Vec::new(), vec![issue]);
        }
    };
    let path: Arc<str> = Arc::from(tree.record_path());
    let mut groups = Vec::new();
    let mut undecided = Undecided::default();
    if !undecided.applies(None, tree.field("Requirement")) {
        return Assorted::new(guid, groups, Vec::new());
    }
    if let Some(Value::Array(items)) = tree.field("GroupedLoadouts") {
        for group in items.iter().filter_map(as_struct) {
            let name = text(group, "Group");
            if !undecided.applies(Some(name), group.field("Requirement")) {
                continue;
            }
            let alternatives = struct_items(group, "Loadouts")
                .map(|loadout| read_loadout(loadout, &path))
                .collect();
            groups.push(Group::new(name.to_owned(), alternatives));
        }
    }
    let issues = undecided.into_note(guid).into_iter().collect();
    Assorted::new(guid, groups, issues)
}

impl LoadoutResolver<'_> {
    /// Returns an issue for each named pick whose group no assortment met while resolving has.
    pub(crate) fn unknown_picks(&self) -> Vec<LoadoutError> {
        self.picks
            .unseen(&self.seen_groups)
            .map(|(group, index)| LoadoutError::UnknownPick {
                group: group.to_owned(),
                index,
            })
            .collect()
    }

    /// Returns the assortment `guid` with every alternative, read once per resolver.
    fn assorted(&mut self, guid: Guid) -> Result<Assorted, LoadoutError> {
        if let Some(assorted) = self.assortments.get(&guid) {
            return Ok(assorted.clone());
        }
        let record = self
            .entities
            .forge()
            .record_by_guid(guid)
            .ok_or(LoadoutError::DanglingReference { guid })?;
        let assorted = read_assorted(&mut self.builder, &record);
        self.assortments.insert(guid, assorted.clone());
        Ok(assorted)
    }

    /// Returns the entries of the assortments `guids`: the alternative the picks of the resolver
    /// choose of each group of each assortment, the first by default, laid over each other in
    /// order.
    ///
    /// An assortment that is missing, cannot be read or is used while it is being resolved, which
    /// is a cycle, is an issue; the others still apply. When none gives anything the entries are
    /// empty and carry the issues.
    pub(crate) fn assortment_entries(&mut self, guids: &[Guid]) -> Entries {
        let mut layers = Vec::new();
        let mut issues = Vec::new();
        let mut resolving = Vec::new();
        for &guid in guids {
            if self.assorting.contains(&guid) {
                issues.push(LoadoutError::Assortment {
                    guid,
                    reason: "it names itself, directly or through another assortment".to_owned(),
                });
                continue;
            }
            match self.assorted(guid) {
                Ok(assorted) => {
                    layers.extend(assorted.layers(&self.picks, &mut self.seen_groups, &mut issues));
                    resolving.push(guid);
                }
                Err(error) => issues.push(error),
            }
        }
        let depth = self.assorting.len();
        self.assorting.extend(resolving);
        let entries = self.layered_entries(&layers);
        self.assorting.truncate(depth);
        let list = match entries {
            Ok(entries) => {
                issues.extend(entries.issues.iter().cloned());
                entries.list
            }
            Err(error) => {
                issues.push(error);
                Arc::from([])
            }
        };
        Entries {
            list,
            issues: issues.into(),
        }
    }
}
