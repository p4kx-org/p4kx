// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_comparison::{SchemaChangeKind, SchemaCounts, SchemaDiff, SideCounts, StructChange};

/// What a row of the schema list describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchemaRowKind {
    /// A struct.
    Struct,
    /// The parent a changed struct inherits from.
    Parent,
    /// A property a changed struct declares.
    Property,
    /// An enum.
    Enum,
    /// An option of a changed enum.
    Option,
}

/// One row of the schema list: a struct, enum, property, parent or enum option that differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaRow {
    kind: SchemaRowKind,
    change: SchemaChangeKind,
    name: String,
    old: String,
    new: String,
    records: Option<SideCounts>,
}

impl SchemaRow {
    /// Returns what the row describes.
    #[must_use]
    pub const fn kind(&self) -> SchemaRowKind {
        self.kind
    }

    /// Returns how the definition differs.
    #[must_use]
    pub const fn change(&self) -> SchemaChangeKind {
        self.change
    }

    /// Returns the name of the struct, enum, property or option; empty for a parent.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the definition on the old side as text, empty when the side lacks it.
    #[must_use]
    pub fn old_text(&self) -> &str {
        &self.old
    }

    /// Returns the definition on the new side as text, empty when the side lacks it.
    #[must_use]
    pub fn new_text(&self) -> &str {
        &self.new
    }

    /// Returns the Records of a struct or enum on each side, `None` for any other row.
    #[must_use]
    pub const fn records(&self) -> Option<SideCounts> {
        self.records
    }

    /// Returns whether the row belongs to the struct or enum above it.
    #[must_use]
    pub const fn is_member(&self) -> bool {
        matches!(
            self.kind,
            SchemaRowKind::Parent | SchemaRowKind::Property | SchemaRowKind::Option
        )
    }
}

/// The schema drift between the DataForges of a finished Comparison as the schema list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaView {
    counts: SchemaCounts,
    rows: Vec<SchemaRow>,
}

impl SchemaView {
    /// Lists the structs, enums and their members of `diff`: each struct and enum is followed by
    /// its changed parent, properties and options.
    #[must_use]
    pub fn new(diff: &SchemaDiff) -> Self {
        let mut rows = Vec::new();
        for change in &diff.structs {
            struct_rows(change, &mut rows);
        }
        for change in &diff.enums {
            rows.push(SchemaRow {
                kind: SchemaRowKind::Enum,
                change: change.change,
                name: change.name.clone(),
                old: String::new(),
                new: String::new(),
                records: Some(change.records),
            });
            rows.extend(change.options.iter().map(|option| SchemaRow {
                kind: SchemaRowKind::Option,
                change: option.change.into(),
                name: option.name.clone(),
                old: String::new(),
                new: String::new(),
                records: None,
            }));
        }
        Self {
            counts: diff.counts,
            rows,
        }
    }

    /// Returns the number of changes per kind and level.
    #[must_use]
    pub const fn counts(&self) -> SchemaCounts {
        self.counts
    }

    /// Returns the number of structs and enums that differ.
    #[must_use]
    pub const fn changed(&self) -> u64 {
        self.counts.structs.total() + self.counts.enums.total()
    }

    /// Returns the rows in list order.
    #[must_use]
    pub fn rows(&self) -> &[SchemaRow] {
        &self.rows
    }
}

fn struct_rows(change: &StructChange, rows: &mut Vec<SchemaRow>) {
    rows.push(SchemaRow {
        kind: SchemaRowKind::Struct,
        change: change.change,
        name: change.name.clone(),
        old: String::new(),
        new: String::new(),
        records: Some(change.records),
    });
    if let Some(parent) = &change.parent {
        rows.push(SchemaRow {
            kind: SchemaRowKind::Parent,
            change: SchemaChangeKind::Changed,
            name: String::new(),
            old: parent.old.clone().unwrap_or_default(),
            new: parent.new.clone().unwrap_or_default(),
            records: None,
        });
    }
    rows.extend(change.properties.iter().map(|property| SchemaRow {
        kind: SchemaRowKind::Property,
        change: property.change,
        name: property.name.clone(),
        old: property.old.as_ref().map_or_default(ToString::to_string),
        new: property.new.as_ref().map_or_default(ToString::to_string),
        records: None,
    }));
}
