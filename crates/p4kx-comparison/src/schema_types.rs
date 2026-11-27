// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;

use schemars::JsonSchema;
use serde::Serialize;

use crate::report::nullable;

/// How a struct, property or enum differs between the two sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum SchemaChangeKind {
    /// Only the new side has it.
    Added,
    /// Only the old side has it.
    Removed,
    /// Both sides have it and it differs.
    Changed,
}

impl fmt::Display for SchemaChangeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Added => "added",
            Self::Removed => "removed",
            Self::Changed => "changed",
        })
    }
}

/// How an enum option differs between the two sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum OptionChangeKind {
    /// Only the new side has the option.
    Added,
    /// Only the old side has the option.
    Removed,
}

impl From<OptionChangeKind> for SchemaChangeKind {
    fn from(kind: OptionChangeKind) -> Self {
        match kind {
            OptionChangeKind::Added => Self::Added,
            OptionChangeKind::Removed => Self::Removed,
        }
    }
}

/// A count for each side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, JsonSchema)]
pub struct SideCounts {
    /// The count on the old side.
    pub old: u64,
    /// The count on the new side.
    pub new: u64,
}

/// The number of definitions per change kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, JsonSchema)]
pub struct KindCounts {
    /// Definitions only the new side has.
    pub added: u64,
    /// Definitions only the old side has.
    pub removed: u64,
    /// Definitions both sides have that differ.
    pub changed: u64,
}

impl KindCounts {
    pub(crate) const fn add(&mut self, kind: SchemaChangeKind) {
        match kind {
            SchemaChangeKind::Added => self.added += 1,
            SchemaChangeKind::Removed => self.removed += 1,
            SchemaChangeKind::Changed => self.changed += 1,
        }
    }

    /// Returns the number of definitions that differ.
    #[must_use]
    pub const fn total(&self) -> u64 {
        self.added + self.removed + self.changed
    }
}

/// The number of changes per kind and level of the schema.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, JsonSchema)]
pub struct SchemaCounts {
    /// The struct changes.
    pub structs: KindCounts,
    /// The property changes in added, removed and changed structs.
    pub properties: KindCounts,
    /// The enum changes.
    pub enums: KindCounts,
    /// The option changes of the changed enums.
    pub options: KindCounts,
}

/// What a property is: its type, conversion and target.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
pub struct PropertyShape {
    /// The data type, e.g. `Reference`, `String` or `Class`; `Unknown(0x…)` for an unknown code.
    pub data_type: String,
    /// The conversion type: `0` for one value, any other code for an array of values.
    pub conversion: u16,
    /// The name of the struct or enum the values are of, point to or refer to; `null` for a
    /// property without a target.
    #[schemars(required, transform = nullable)]
    pub target: Option<String>,
}

impl fmt::Display for PropertyShape {
    /// Writes the data type, `[]` for an array and the target, e.g. `Reference Part` or
    /// `String[]`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.data_type)?;
        if self.conversion != 0 {
            f.write_str("[]")?;
        }
        if let Some(target) = &self.target {
            write!(f, " {target}")?;
        }
        Ok(())
    }
}

/// An aspect of a property that differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PropertyAspect {
    /// The data type.
    DataType,
    /// The conversion type.
    Conversion,
    /// The target struct or enum.
    Target,
}

impl fmt::Display for PropertyAspect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::DataType => "type",
            Self::Conversion => "conversion",
            Self::Target => "target",
        })
    }
}

/// One added, removed or changed property declared by a compared struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct PropertyChange {
    /// The property name.
    pub name: String,
    /// How the property differs.
    pub change: SchemaChangeKind,
    /// What differs, in the order data type, conversion, target; empty for an added or removed
    /// property.
    pub aspects: Vec<PropertyAspect>,
    /// The property on the old side, absent when it was added.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<PropertyShape>,
    /// The property on the new side, absent when it was removed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<PropertyShape>,
}

/// The parent of a struct on each side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ParentChange {
    /// The parent on the old side, `null` for none.
    #[schemars(required, transform = nullable)]
    pub old: Option<String>,
    /// The parent on the new side, `null` for none.
    #[schemars(required, transform = nullable)]
    pub new: Option<String>,
}

/// One added, removed or changed struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct StructChange {
    /// The struct name.
    pub name: String,
    /// How the struct differs.
    pub change: SchemaChangeKind,
    /// The Records on each side whose struct is this struct or inherits from it; `0` on the side
    /// that lacks the struct.
    pub records: SideCounts,
    /// The directly mapped instances of this struct on each side, including nested instances.
    /// An absent struct has a count of `0`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mapped_instances: Option<SideCounts>,
    /// The parents, present when a changed struct inherits from another struct than before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<ParentChange>,
    /// The properties the struct declares itself that differ, ordered by name.
    pub properties: Vec<PropertyChange>,
}

/// One added or removed option of a changed enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct OptionChange {
    /// The option name.
    pub name: String,
    /// `added` or `removed`.
    pub change: OptionChangeKind,
}

/// One added, removed or changed enum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct EnumChange {
    /// The enum name.
    pub name: String,
    /// How the enum differs.
    pub change: SchemaChangeKind,
    /// The Records on each side whose struct declares or inherits a property of this enum, each
    /// Record counted once.
    pub records: SideCounts,
    /// The options added or removed, ordered by name; empty for an added or removed enum.
    pub options: Vec<OptionChange>,
}

/// The schema drift between two DataForges: the structs, properties, enums and enum options added,
/// removed and changed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct SchemaDiff {
    /// The number of changes per kind and level.
    pub counts: SchemaCounts,
    /// Every added, removed or changed struct, ordered by name.
    pub structs: Vec<StructChange>,
    /// Every added, removed or changed enum, ordered by name.
    pub enums: Vec<EnumChange>,
    /// The definitions, properties, options and Records on each side that were left out because a
    /// name or struct cannot be read or a name is defined twice.
    pub unreadable: SideCounts,
}

impl SchemaDiff {
    /// Returns whether the schemas differ: a struct or enum changed, or a different number of
    /// definitions was left out on each side.
    #[must_use]
    pub const fn has_differences(&self) -> bool {
        !self.structs.is_empty()
            || !self.enums.is_empty()
            || self.unreadable.old != self.unreadable.new
    }
}
