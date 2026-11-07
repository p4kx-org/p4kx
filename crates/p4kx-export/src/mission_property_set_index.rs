// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::fmt::{self, Write};
use std::hash::{Hash, Hasher};

use p4kx_extraction::{StructValue, Value};

struct SourceNode<'t, 'f>(&'t StructValue<'f>);

impl Hash for SourceNode<'_, '_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.0, state);
    }
}

impl PartialEq for SourceNode<'_, '_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for SourceNode<'_, '_> {}

/// Canonical authored locations of borrowed source nodes that own property overrides.
pub(crate) struct MissionPropertySetIndex<'t, 'f> {
    locations: HashMap<SourceNode<'t, 'f>, String>,
}

impl<'t, 'f> MissionPropertySetIndex<'t, 'f> {
    /// Indexes source nodes while retaining the borrow that keeps their identity stable.
    pub(crate) fn new(root: &'t StructValue<'f>) -> Result<Self, fmt::Error> {
        let mut locations = HashMap::new();
        visit(root, &mut String::new(), &mut locations)?;
        Ok(Self { locations })
    }

    /// Returns the node's authored location without resolving names or equivalent content.
    pub(crate) fn location(&self, node: &'t StructValue<'f>) -> Option<&str> {
        self.locations.get(&SourceNode(node)).map(String::as_str)
    }
}

fn visit<'t, 'f>(
    value: &'t StructValue<'f>,
    path: &mut String,
    locations: &mut HashMap<SourceNode<'t, 'f>, String>,
) -> fmt::Result {
    if value.field("propertyOverrides").is_some() {
        locations.insert(SourceNode(value), path.clone());
    }
    for field in value.fields() {
        let length = path.len();
        if !path.is_empty() {
            path.push('.');
        }
        path.push_str(field.name());
        visit_value(field.value(), path, locations)?;
        path.truncate(length);
    }
    Ok(())
}

fn visit_value<'t, 'f>(
    value: &'t Value<'f>,
    path: &mut String,
    locations: &mut HashMap<SourceNode<'t, 'f>, String>,
) -> fmt::Result {
    match value {
        Value::Struct(value) => visit(value, path, locations)?,
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                let length = path.len();
                write!(path, "[{index}]")?;
                visit_value(value, path, locations)?;
                path.truncate(length);
            }
        }
        _ => {}
    }
    Ok(())
}
