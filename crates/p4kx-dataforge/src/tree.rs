// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Field, Guid, Location, RecordMetadata, Step, StructValue, Value, ValueError};

/// The canonical tree of one Record: its GUID, record path, name and root struct with the fields.
///
/// It is the single model every output is made from; text is borrowed from the DataForge.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordTree<'f> {
    pub(crate) record_index: u32,
    pub(crate) metadata: RecordMetadata<'f>,
    pub(crate) guid: Guid,
    pub(crate) record_path: &'f str,
    pub(crate) name: &'f str,
    pub(crate) tag: Option<&'f str>,
    pub(crate) root: StructValue<'f>,
}

impl<'f> RecordTree<'f> {
    /// Returns original borrowed metadata independently of normalized legacy header getters.
    #[must_use]
    pub const fn metadata(&self) -> RecordMetadata<'f> {
        self.metadata
    }

    /// Returns the owning Record position in source definition order.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }

    /// Returns the name of the Record's struct, e.g. `EntityClassDefinition`.
    #[must_use]
    pub const fn type_name(&self) -> &'f str {
        self.root.type_name
    }

    /// Returns the GUID that identifies the Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the record path, without `.xml`.
    #[must_use]
    pub const fn record_path(&self) -> &'f str {
        self.record_path
    }

    /// Returns the Record name, e.g. `EntityClassDefinition.AEGS_Avenger`.
    #[must_use]
    pub const fn name(&self) -> &'f str {
        self.name
    }

    /// Returns the Record tag, e.g. `SystemsDesign`; `None` for a version 6 DataForge and for a
    /// Record with an empty or no tag.
    #[must_use]
    pub const fn tag(&self) -> Option<&'f str> {
        self.tag
    }

    /// Returns the Record's own struct, to walk it like any nested struct.
    #[must_use]
    pub const fn root(&self) -> &StructValue<'f> {
        &self.root
    }

    /// Returns the fields in declaration order, the inherited ones first.
    #[must_use]
    pub fn fields(&self) -> &[Field<'f>] {
        &self.root.fields
    }

    /// Returns the value of the first field called `name`, or `None` when there is none.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Value<'f>> {
        self.root.field(name)
    }

    /// Returns every error value in the tree, nested ones included, with the name of the field
    /// that holds it, in the order the tree lists them.
    pub fn errors(&self) -> impl Iterator<Item = (&'f str, &ValueError)> {
        self.located_errors().map(|(location, error)| {
            let name = location.steps().iter().rev().find_map(|step| match step {
                Step::Field(name) => Some(*name),
                Step::Element(_) => None,
            });
            (name.unwrap_or_default(), error)
        })
    }

    /// Returns every error value in the tree, nested ones included, with its full location, in
    /// the order the tree lists them.
    pub fn located_errors(&self) -> impl Iterator<Item = (Location<'f>, &ValueError)> {
        let mut errors = Vec::new();
        let mut steps = Vec::new();
        collect_errors(&self.root.fields, &mut steps, &mut errors);
        errors.into_iter()
    }

    /// Returns the number of error values in the tree, nested ones included, without collecting
    /// them.
    #[must_use]
    pub fn error_count(&self) -> usize {
        count_field_errors(&self.root.fields)
    }
}

fn count_field_errors(fields: &[Field<'_>]) -> usize {
    fields.iter().map(|field| count_errors(&field.value)).sum()
}

fn count_errors(value: &Value<'_>) -> usize {
    match value {
        Value::Error(_) => 1,
        Value::Struct(structure) => count_field_errors(&structure.fields),
        Value::Array(items) => items.iter().map(count_errors).sum(),
        _ => 0,
    }
}

type Located<'t, 'f> = Vec<(Location<'f>, &'t ValueError)>;

fn collect_errors<'t, 'f>(
    fields: &'t [Field<'f>],
    steps: &mut Vec<Step<'f>>,
    errors: &mut Located<'t, 'f>,
) {
    for field in fields {
        steps.push(Step::Field(field.name));
        collect_value_errors(&field.value, steps, errors);
        steps.pop();
    }
}

fn collect_value_errors<'t, 'f>(
    value: &'t Value<'f>,
    steps: &mut Vec<Step<'f>>,
    errors: &mut Located<'t, 'f>,
) {
    match value {
        Value::Error(error) => errors.push((Location::new(steps), error)),
        Value::Struct(structure) => collect_errors(&structure.fields, steps, errors),
        Value::Array(items) => {
            for (index, item) in (0_u32..).zip(items.iter()) {
                steps.push(Step::Element(index));
                collect_value_errors(item, steps, errors);
                steps.pop();
            }
        }
        _ => {}
    }
}
