// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Guid, StructValue, Value};

use crate::reader::{as_struct, int, items, reference, text};

const PREFIX: &str = "BuildingBlocks_StyleSelectorCondition";
const MAX_CONDITION_DEPTH: usize = 8;
const MAX_LINK_DEPTH: usize = 4;
const MAX_COUNT: u32 = 1 << 20;

/// Which siblings a [`Condition::Count`] selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CountKind {
    First,
    Every,
}

impl CountKind {
    fn parse(name: Option<&str>) -> Option<Self> {
        match name {
            Some("First") => Some(Self::First),
            Some("Every") => Some(Self::Every),
            _ => None,
        }
    }
}

/// A selector of a style entry.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Condition {
    Tag(Guid),
    AnyOfTag(Vec<Guid>),
    AllOfTag(Vec<Guid>),
    NotTag(Guid),
    Type(String),
    Parent(Vec<Self>),
    Ancestor {
        conditions: Vec<Self>,
        breaks: Vec<Self>,
    },
    Child(Vec<Self>),
    Count {
        kind: CountKind,
        count: u32,
        offset: u32,
    },
    AllOf(Vec<Self>),
    AnyOf(Vec<Self>),
    Unsupported(String),
}

/// Which selectors that need the shape of the tree an entry holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Uses {
    pub(crate) count: bool,
    pub(crate) child: bool,
}

impl Uses {
    pub(crate) const fn or(self, other: Self) -> Self {
        Self {
            count: self.count || other.count,
            child: self.child || other.child,
        }
    }

    pub(crate) fn of(conditions: &[Condition]) -> Self {
        conditions.iter().fold(Self::default(), |found, condition| {
            found.or(Self::of_one(condition))
        })
    }

    fn of_one(condition: &Condition) -> Self {
        match condition {
            Condition::Count { .. } => Self {
                count: true,
                child: false,
            },
            Condition::Child(inner) => Self {
                count: false,
                child: true,
            }
            .or(Self::of(inner)),
            Condition::Ancestor { conditions, breaks } => Self::of(conditions).or(Self::of(breaks)),
            Condition::Parent(inner) | Condition::AllOf(inner) | Condition::AnyOf(inner) => {
                Self::of(inner)
            }
            _ => Self::default(),
        }
    }
}

/// Returns whether the sibling at `position` is one that a `Count` selects: `First` the `count`
/// siblings from `offset` on, `Every` each `count`-th sibling from `offset` on.
pub(crate) fn counted(kind: CountKind, count: u32, offset: u32, position: usize) -> bool {
    let (Ok(count), Ok(offset)) = (usize::try_from(count), usize::try_from(offset)) else {
        return false;
    };
    let Some(from) = position.checked_sub(offset) else {
        return false;
    };
    match kind {
        CountKind::First => from < count,
        CountKind::Every => count > 0 && from % count == 0,
    }
}

/// Returns whether `conditions` require the widget itself to be of the type `kind`, directly or
/// inside an all-of; types asked of a parent, ancestor or child do not count.
fn requires_type(conditions: &[Condition], kind: &str) -> bool {
    any_in_all_of(
        conditions,
        &|condition| matches!(condition, Condition::Type(found) if found == kind),
    )
}

/// Returns whether `conditions` name a type of the widget itself, directly or inside an all-of.
fn names_type(conditions: &[Condition]) -> bool {
    any_in_all_of(conditions, &|condition| {
        matches!(condition, Condition::Type(_))
    })
}

/// Returns whether `conditions` require a parent that meets some condition, directly or inside
/// an all-of.
fn requires_parent(conditions: &[Condition]) -> bool {
    any_in_all_of(conditions, &|condition| {
        matches!(condition, Condition::Parent(_))
    })
}

/// Returns whether `test` holds for a condition of the list or of an all-of inside it.
fn any_in_all_of(conditions: &[Condition], test: &dyn Fn(&Condition) -> bool) -> bool {
    conditions.iter().any(|condition| match condition {
        Condition::AllOf(inner) => any_in_all_of(inner, test),
        other => test(other),
    })
}

/// Returns whether `conditions` select the inner part of a widget, such as the text of a text
/// field or the shape of an icon, a widget of the type `kind` below it: they require that type,
/// or name no type and select by the parent.
pub(crate) fn selects_part(conditions: &[Condition], kind: &str) -> bool {
    requires_type(conditions, kind) || (requires_parent(conditions) && !names_type(conditions))
}

pub(crate) fn collect_unsupported(condition: &Condition, out: &mut Vec<String>) {
    match condition {
        Condition::Unsupported(name) => {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
        Condition::Parent(inner)
        | Condition::Child(inner)
        | Condition::AllOf(inner)
        | Condition::AnyOf(inner) => {
            for condition in inner {
                collect_unsupported(condition, out);
            }
        }
        Condition::Ancestor { conditions, breaks } => {
            for condition in conditions.iter().chain(breaks) {
                collect_unsupported(condition, out);
            }
        }
        _ => {}
    }
}

#[derive(Clone, Copy)]
struct Depth {
    all: usize,
    links: usize,
}

pub(crate) fn conditions(values: &[Value<'_>]) -> Vec<Condition> {
    conditions_at(values, Depth { all: 0, links: 0 })
}

fn conditions_at(values: &[Value<'_>], depth: Depth) -> Vec<Condition> {
    values
        .iter()
        .filter_map(as_struct)
        .map(|structure| condition(structure, depth))
        .collect()
}

fn guid_of(structure: &StructValue<'_>, name: &str) -> Option<Guid> {
    reference(structure, name).map(p4kx_dataforge::Reference::guid)
}

fn guids_of(structure: &StructValue<'_>, name: &str) -> Vec<Guid> {
    items(structure, name)
        .iter()
        .filter_map(|value| match value {
            Value::Reference(reference) => Some(reference.guid()),
            _ => None,
        })
        .collect()
}

fn holds_link(conditions: &[Condition]) -> bool {
    conditions.iter().any(|condition| match condition {
        Condition::Parent(_) | Condition::Ancestor { .. } => true,
        Condition::AllOf(inner) | Condition::AnyOf(inner) | Condition::Child(inner) => {
            holds_link(inner)
        }
        _ => false,
    })
}

fn count_of(structure: &StructValue<'_>, name: &str) -> u32 {
    int(structure, name)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0)
        .min(MAX_COUNT)
}

fn condition(structure: &StructValue<'_>, depth: Depth) -> Condition {
    let kind = structure
        .type_name()
        .strip_prefix(PREFIX)
        .unwrap_or_else(|| structure.type_name());
    let link = matches!(kind, "Parent" | "Ancestor");
    if depth.all >= MAX_CONDITION_DEPTH || (link && depth.links >= MAX_LINK_DEPTH) {
        return Condition::Unsupported(format!("{kind} nested too deep"));
    }
    let deeper = Depth {
        all: depth.all + 1,
        links: depth.links + usize::from(link),
    };
    let nested = || conditions_at(items(structure, "conditions"), deeper);
    match kind {
        "Tag" => guid_of(structure, "tag")
            .map_or_else(|| Condition::Unsupported("Tag".to_owned()), Condition::Tag),
        "NotTag" => guid_of(structure, "tag").map_or_else(
            || Condition::Unsupported("NotTag".to_owned()),
            Condition::NotTag,
        ),
        "AnyOfTag" => Condition::AnyOfTag(guids_of(structure, "tags")),
        "AllOfTag" => Condition::AllOfTag(guids_of(structure, "tags")),
        "Type" => Condition::Type(text(structure, "type").unwrap_or_default().to_owned()),
        "Parent" => Condition::Parent(nested()),
        "Ancestor" => Condition::Ancestor {
            conditions: nested(),
            breaks: conditions_at(items(structure, "breakConditions"), deeper),
        },
        "Child" => {
            let inner = nested();
            if holds_link(&inner) {
                Condition::Unsupported("Child with a Parent or Ancestor inside".to_owned())
            } else {
                Condition::Child(inner)
            }
        }
        "Count" => CountKind::parse(text(structure, "type")).map_or_else(
            || {
                Condition::Unsupported(format!(
                    "Count of type {}",
                    text(structure, "type").unwrap_or("?")
                ))
            },
            |kind| Condition::Count {
                kind,
                count: count_of(structure, "count"),
                offset: count_of(structure, "offset"),
            },
        ),
        "AllOfCondition" => Condition::AllOf(nested()),
        "AnyOfCondition" => Condition::AnyOf(nested()),
        other => Condition::Unsupported(other.to_owned()),
    }
}
