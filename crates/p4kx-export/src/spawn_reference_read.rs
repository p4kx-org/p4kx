// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Reference, StructValue, Value};

use super::Reader;
use crate::mission_layers::{items, one};
use crate::record_fields::text;
use crate::spawn_model::TagPaths;
use crate::spawn_tag_text::SpawnTagText;

impl<'x, 'f> Reader<'x, 'f> {
    /// Returns whether a weak reference names the original owning Record.
    pub(crate) fn owns(&self, target: &Reference<'_>) -> bool {
        self.owner == target.record_index() && self.owner_guid == target.guid()
    }

    /// Returns an interpreted weak target within this Record.
    pub(crate) fn owned_target(&self, target: &Reference<'_>) -> Option<&'x StructValue<'f>> {
        if !self.owns(target) {
            return None;
        }
        self.target(target.location()?.as_str())
    }
    /// Returns the tag paths of the `TagList` struct in the field `name`.
    pub(crate) fn tag_list(&self, parent: Option<&StructValue<'f>>, name: &str) -> TagPaths {
        self.tag_array(one(parent, name), "tags")
    }

    /// Returns the tag paths of the array of tag References in the field `name`.
    pub(crate) fn tag_array(&self, parent: Option<&StructValue<'f>>, name: &str) -> TagPaths {
        items(parent, name)
            .iter()
            .filter_map(|tag| self.path(tag).map(SpawnTagText::owned))
            .collect()
    }

    /// Returns the tag path of the tag Reference in the field `name`.
    pub(crate) fn tag_path(&self, parent: Option<&StructValue<'f>>, name: &str) -> Option<String> {
        self.path(parent?.field(name)?)
    }

    /// Returns the path of the tag `value` links to; the GUID text for a tag the tag database
    /// does not hold.
    pub(super) fn path(&self, value: &Value<'f>) -> Option<String> {
        match value {
            Value::Reference(target) if target.location().is_none() => Some(
                self.tags
                    .path_of(target.guid())
                    .map_or_else(|| target.guid().to_string(), str::to_owned),
            ),
            _ => None,
        }
    }

    /// Returns the `missionVariableName` of every property the weak pointers in the array `name`
    /// point to inside this Record, in stored order; a pointer to nothing is left out.
    pub(crate) fn property_variables(
        &self,
        parent: Option<&StructValue<'f>>,
        name: &str,
    ) -> Vec<String> {
        items(parent, name)
            .iter()
            .filter_map(|pointer| self.variable_of(pointer))
            .collect()
    }

    pub(super) fn variable_of(&self, pointer: &Value<'f>) -> Option<String> {
        let property = self.target(self.place(pointer)?)?;
        text(Some(property), "missionVariableName").map(str::to_owned)
    }

    /// Returns where inside this Record the weak pointer `pointer` points.
    fn place<'v>(&self, pointer: &'v Value<'f>) -> Option<&'v str> {
        let Value::Reference(target) = pointer else {
            return None;
        };
        if !self.owns(target) {
            return None;
        }
        let location = target.location()?;
        Some(location.as_str())
    }

    /// Returns the struct at `place` inside this Record.
    fn target(&self, place: &str) -> Option<&'x StructValue<'f>> {
        match place
            .split('.')
            .try_fold(Cursor::Struct(self.root), Cursor::step)?
        {
            Cursor::Struct(found) => Some(found),
            Cursor::Array(_) => None,
        }
    }
}

/// A position while following the text of a location: a struct or an array of a field.
enum Cursor<'t, 'f> {
    Struct(&'t StructValue<'f>),
    Array(&'t [Value<'f>]),
}

impl Cursor<'_, '_> {
    /// Follows `step`, a field name followed by any number of `[index]`.
    fn step(self, step: &str) -> Option<Self> {
        let (field, mut indices) = step.split_once('[').map_or((step, ""), |(field, rest)| {
            (field, rest.trim_end_matches(']'))
        });
        let Self::Struct(parent) = self else {
            return None;
        };
        let mut at = match parent.field(field)? {
            Value::Struct(value) => Self::Struct(value),
            Value::Array(values) => Self::Array(values),
            _ => return None,
        };
        while !indices.is_empty() {
            let (index, rest) = indices.split_once("][").unwrap_or((indices, ""));
            let Self::Array(values) = at else {
                return None;
            };
            at = match values.get(index.parse::<usize>().ok()?)? {
                Value::Struct(value) => Self::Struct(value),
                Value::Array(values) => Self::Array(values),
                _ => return None,
            };
            indices = rest;
        }
        Some(at)
    }
}
