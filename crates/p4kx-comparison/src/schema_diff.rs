// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::schema_side::{EnumDef, SideSchema, StructDef};
use crate::{
    DataForge, EnumChange, OptionChange, OptionChangeKind, ParentChange, PropertyAspect,
    PropertyChange, PropertyShape, SchemaChangeKind, SchemaDiff, SideCounts, StructChange,
};

/// Compares the schema of the `old` with the `new` DataForge: its structs with the properties
/// they declare, and its enums with their options, joined by name.
///
/// A property differs in its data type, conversion type or target; a struct also in its parent.
/// The Records of a struct are those whose struct is it or inherits from it; those of an enum are
/// the Records whose struct declares or inherits a property of it.
#[must_use]
pub fn compare_schema(old: &DataForge<'_>, new: &DataForge<'_>) -> SchemaDiff {
    let (old, new) = rayon::join(|| SideSchema::read(old), || SideSchema::read(new));
    compare_sides(&old, &new)
}

pub(crate) fn compare_sides(old: &SideSchema<'_>, new: &SideSchema<'_>) -> SchemaDiff {
    let mut diff = SchemaDiff {
        unreadable: SideCounts {
            old: old.unreadable,
            new: new.unreadable,
        },
        ..SchemaDiff::default()
    };
    diff.structs = join(&old.structs, &new.structs, struct_change);
    diff.enums = join(&old.enums, &new.enums, enum_change);
    for change in &diff.structs {
        diff.counts.structs.add(change.change);
        for property in &change.properties {
            diff.counts.properties.add(property.change);
        }
    }
    for change in &diff.enums {
        diff.counts.enums.add(change.change);
        for option in &change.options {
            diff.counts.options.add(option.change.into());
        }
    }
    diff
}

fn join<T, C>(
    old: &BTreeMap<&str, T>,
    new: &BTreeMap<&str, T>,
    change: impl Fn(&str, Option<&T>, Option<&T>) -> Option<C>,
) -> Vec<C> {
    let mut changes = Vec::new();
    let mut before = old.iter().peekable();
    let mut after = new.iter().peekable();
    loop {
        let (name, in_old, in_new) = match (before.peek(), after.peek()) {
            (None, None) => break,
            (Some(&(name, _)), None) => (*name, true, false),
            (None, Some(&(name, _))) => (*name, false, true),
            (Some(&(old_name, _)), Some(&(new_name, _))) => match old_name.cmp(new_name) {
                Ordering::Less => (*old_name, true, false),
                Ordering::Greater => (*new_name, false, true),
                Ordering::Equal => (*old_name, true, true),
            },
        };
        let old_value = in_old.then(|| before.next()).flatten().map(|(_, v)| v);
        let new_value = in_new.then(|| after.next()).flatten().map(|(_, v)| v);
        changes.extend(change(name, old_value, new_value));
    }
    changes
}

fn struct_change(
    name: &str,
    old: Option<&StructDef<'_>>,
    new: Option<&StructDef<'_>>,
) -> Option<StructChange> {
    let records = SideCounts {
        old: old.map_or(0, |def| def.records),
        new: new.map_or(0, |def| def.records),
    };
    let mapped_instances = SideCounts {
        old: old.map_or(0, |def| def.mapped_instances),
        new: new.map_or(0, |def| def.mapped_instances),
    };
    let empty = BTreeMap::new();
    let properties = join(
        old.map_or(&empty, |def| &def.properties),
        new.map_or(&empty, |def| &def.properties),
        |name, old, new| {
            property_change(name, old.map(|def| &def.shape), new.map(|def| &def.shape))
        },
    );
    let (change, parent) = match (old, new) {
        (Some(_), None) => (SchemaChangeKind::Removed, None),
        (None, Some(_)) => (SchemaChangeKind::Added, None),
        (Some(old), Some(new)) => {
            let parent = (old.parent != new.parent).then(|| ParentChange {
                old: old.parent.map(str::to_owned),
                new: new.parent.map(str::to_owned),
            });
            if parent.is_none() && properties.is_empty() {
                return None;
            }
            (SchemaChangeKind::Changed, parent)
        }
        (None, None) => return None,
    };
    Some(StructChange {
        name: name.to_owned(),
        change,
        records,
        mapped_instances: Some(mapped_instances),
        parent,
        properties,
    })
}

fn property_change(
    name: &str,
    old: Option<&PropertyShape>,
    new: Option<&PropertyShape>,
) -> Option<PropertyChange> {
    let (change, aspects) = match (old, new) {
        (Some(_), None) => (SchemaChangeKind::Removed, Vec::new()),
        (None, Some(_)) => (SchemaChangeKind::Added, Vec::new()),
        (Some(old), Some(new)) => {
            let aspects: Vec<PropertyAspect> = [
                (old.data_type != new.data_type, PropertyAspect::DataType),
                (old.conversion != new.conversion, PropertyAspect::Conversion),
                (old.target != new.target, PropertyAspect::Target),
            ]
            .into_iter()
            .filter_map(|(differs, aspect)| differs.then_some(aspect))
            .collect();
            if aspects.is_empty() {
                return None;
            }
            (SchemaChangeKind::Changed, aspects)
        }
        (None, None) => return None,
    };
    Some(PropertyChange {
        name: name.to_owned(),
        change,
        aspects,
        old: old.cloned(),
        new: new.cloned(),
    })
}

fn enum_change(
    name: &str,
    old: Option<&EnumDef<'_>>,
    new: Option<&EnumDef<'_>>,
) -> Option<EnumChange> {
    let records = SideCounts {
        old: old.map_or(0, |def| def.records),
        new: new.map_or(0, |def| def.records),
    };
    let (change, options) = match (old, new) {
        (Some(_), None) => (SchemaChangeKind::Removed, Vec::new()),
        (None, Some(_)) => (SchemaChangeKind::Added, Vec::new()),
        (Some(old), Some(new)) => {
            let options = join(&old.options, &new.options, |name, old, new| {
                let change = match (old, new) {
                    (Some(_), None) => OptionChangeKind::Removed,
                    (None, Some(_)) => OptionChangeKind::Added,
                    _ => return None,
                };
                Some(OptionChange {
                    name: name.to_owned(),
                    change,
                })
            });
            if options.is_empty() {
                return None;
            }
            (SchemaChangeKind::Changed, options)
        }
        (None, None) => return None,
    };
    Some(EnumChange {
        name: name.to_owned(),
        change,
        records,
        options,
    })
}
