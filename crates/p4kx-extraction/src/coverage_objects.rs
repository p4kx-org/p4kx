// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_dataforge::{DataForge, Guid, RecordTree, TreeBuilder, Value};

use crate::coverage::{ObjectCoverage, ReferenceItem, SampleExport};
use crate::coverage_forge::Tally;

pub(crate) const STAR_MAP_OBJECT: &str = "StarMapObject";
const TYPE_FIELD: &str = "type";

/// The slots of the listed `StarMapObjectType` names and, per type Record already looked at, the
/// slot of its name.
pub(crate) struct TypeSlots {
    cache: HashMap<Guid, Option<usize>>,
}

impl TypeSlots {
    pub(crate) fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    /// Returns the slot of the listed name of the `StarMapObjectType` Record the Reference `type`
    /// of `tree` links to; `None` when the Reference is missing, dangling or names no listed type.
    pub(crate) fn slot<'f>(
        &mut self,
        forge: &'f DataForge<'f>,
        names: &HashMap<&str, usize>,
        builder: &mut TreeBuilder<'f>,
        tree: &RecordTree<'f>,
    ) -> Option<usize> {
        let Some(Value::Reference(reference)) = tree.field(TYPE_FIELD) else {
            return None;
        };
        if reference.location().is_some() {
            return None;
        }
        let guid = reference.guid();
        if let Some(slot) = self.cache.get(&guid) {
            return *slot;
        }
        let slot = forge
            .record_by_guid(guid)
            .and_then(|record| builder.build(&record).ok())
            .and_then(|target| names.get(type_name(&target)).copied());
        self.cache.insert(guid, slot);
        slot
    }
}

/// Returns the plain `name` field of a `StarMapObjectType` Record, else its Record name without
/// the leading `<type>.`.
fn type_name<'f>(target: &RecordTree<'f>) -> &'f str {
    if let Some(Value::String(name)) = target.field("name")
        && !name.is_empty()
        && !name.starts_with('@')
    {
        return name;
    }
    let name = target.name();
    name.strip_prefix(target.type_name())
        .and_then(|rest| rest.strip_prefix('.'))
        .unwrap_or(name)
}

/// Returns the coverage of every listed `StarMapObjectType` name in list order.
pub(crate) fn coverage(
    items: &[ReferenceItem<'_>],
    names: &HashMap<&str, usize>,
    tallies: &[Tally],
    mut read_sample: impl FnMut(usize, u32) -> SampleExport,
) -> Vec<ObjectCoverage> {
    items
        .iter()
        .enumerate()
        .map(|(row, item)| {
            let tally = names
                .get(item.name)
                .and_then(|first| tallies.get(*first))
                .copied()
                .unwrap_or_default();
            ObjectCoverage {
                area: item.area.to_owned(),
                type_name: item.name.to_owned(),
                records: tally.records,
                templates: tally.templates,
                sample: tally.sample.map(|index| read_sample(row, index)),
            }
        })
        .collect()
}
