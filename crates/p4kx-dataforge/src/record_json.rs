// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::sink::{self, Out};
use super::tasks::{Fields, Items, Selected, Task};
use crate::{Annotations, Field, RecordTree, Value};
use crate::{field_escapes, value_json, xml};
use std::io::{self, Write};

/// The layout of the same Record JSON value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JsonLayout {
    /// Two-space indentation and a final newline.
    #[default]
    Pretty,
    /// One line without whitespace or a line ending.
    Compact,
}

/// Writes canonical Record JSON and retains its field-classification scratch.
#[derive(Debug, Default)]
pub struct RecordJson {
    layout: JsonLayout,
    attribute: Vec<bool>,
    escaped: Vec<bool>,
    order: Vec<usize>,
}

impl RecordJson {
    /// Creates a writer for the chosen layout.
    #[must_use]
    pub fn new(layout: JsonLayout) -> Self {
        Self {
            layout,
            ..Self::default()
        }
    }

    /// Returns the configured layout.
    #[must_use]
    pub const fn layout(&self) -> JsonLayout {
        self.layout
    }

    /// Writes one canonical Record JSON object.
    ///
    /// # Errors
    /// Returns the first output I/O error.
    pub fn write<W: Write>(&mut self, tree: &RecordTree<'_>, out: &mut W) -> io::Result<()> {
        self.write_annotated(tree, Annotations::none(), out)
    }

    /// Writes one Record JSON object with the supplied text and tag annotations.
    ///
    /// # Errors
    /// Returns the first output I/O error.
    pub fn write_annotated<W: Write>(
        &mut self,
        tree: &RecordTree<'_>,
        annotations: Annotations<'_>,
        out: &mut W,
    ) -> io::Result<()> {
        self.attribute.clear();
        self.escaped.clear();
        let pretty = self.layout == JsonLayout::Pretty;
        let mut out = Out {
            sink: out,
            pretty,
            annotations,
        };
        let mut pending = vec![Task::Structure {
            record: Some(tree),
            type_name: tree.type_name(),
            fields: tree.fields(),
            depth: 0,
        }];
        while let Some(task) = pending.pop() {
            self.step(&mut out, task, &mut pending)?;
        }
        if pretty {
            out.sink.write_all(b"\n")?;
        }
        Ok(())
    }

    fn step<'tree, 'source, W: Write>(
        &mut self,
        out: &mut Out<'_, W>,
        task: Task<'tree, 'source>,
        pending: &mut Vec<Task<'tree, 'source>>,
    ) -> io::Result<()> {
        match task {
            Task::Structure {
                record,
                type_name,
                fields,
                depth,
            } => {
                self.structure(out, record, type_name, fields, depth, pending)?;
            }
            Task::Fields(fields) => self.fields(out, fields, pending)?,
            Task::Items(items) => Self::items(out, items, pending)?,
            Task::Value(value, depth) => Self::value(out, value, depth, pending)?,
            Task::CloseWrapper(depth) => out.close(depth, b"}")?,
        }
        Ok(())
    }

    fn structure<'tree, 'source, W: Write>(
        &mut self,
        out: &mut Out<'_, W>,
        record: Option<&'tree RecordTree<'source>>,
        type_name: &'source str,
        fields: &'tree [Field<'source>],
        depth: usize,
        pending: &mut Vec<Task<'tree, 'source>>,
    ) -> io::Result<()> {
        sink::header(out, record, type_name, depth)?;
        let attributes = self.attribute.len();
        xml::classify(fields, &mut self.attribute, &mut self.order);
        let escaped = self.escaped.len();
        field_escapes::append(fields, &mut self.escaped, &mut self.order);
        pending.push(Task::Fields(Fields::new(
            fields, attributes, escaped, depth,
        )));
        Ok(())
    }

    fn fields<'tree, 'source, W: Write>(
        &mut self,
        out: &mut Out<'_, W>,
        mut fields: Fields<'tree, 'source>,
        pending: &mut Vec<Task<'tree, 'source>>,
    ) -> io::Result<()> {
        if let Some(selected) = fields.next(&self.attribute, &self.escaped) {
            let depth = fields.depth;
            pending.push(Task::Fields(fields));
            Self::field(out, selected, depth, pending)?;
        } else {
            self.attribute.truncate(fields.attributes);
            self.escaped.truncate(fields.escaped);
            out.close(fields.depth, b"}")?;
        }
        Ok(())
    }

    fn items<'tree, 'source, W: Write>(
        out: &mut Out<'_, W>,
        mut items: Items<'tree, 'source>,
        pending: &mut Vec<Task<'tree, 'source>>,
    ) -> io::Result<()> {
        if let Some(value) = items.next() {
            out.separator(items.first, items.depth + 1)?;
            items.first = false;
            let depth = items.depth + 1;
            pending.push(Task::Items(items));
            pending.push(Task::Value(value, depth));
        } else if items.first {
            out.sink.write_all(b"]")?;
        } else {
            out.close(items.depth, b"]")?;
        }
        Ok(())
    }

    fn value<'tree, 'source, W: Write>(
        out: &mut Out<'_, W>,
        value: &'tree Value<'source>,
        depth: usize,
        pending: &mut Vec<Task<'tree, 'source>>,
    ) -> io::Result<()> {
        match value {
            Value::Struct(structure) => pending.push(Task::Structure {
                record: None,
                type_name: structure.type_name(),
                fields: structure.fields(),
                depth,
            }),
            Value::Array(items) => {
                out.sink.write_all(b"[")?;
                pending.push(Task::Items(Items::new(items, depth)));
            }
            leaf => sink::leaf(out, leaf, depth)?,
        }
        Ok(())
    }
    fn field<'tree, 'source, W: Write>(
        out: &mut Out<'_, W>,
        selected: Selected<'tree, 'source>,
        depth: usize,
        pending: &mut Vec<Task<'tree, 'source>>,
    ) -> io::Result<()> {
        if selected.escaped {
            out.separator(false, depth + 1)?;
            write!(out.sink, "\"${}\":", selected.index)?;
            out.space()?;
            out.sink.write_all(b"{")?;
            out.key(true, depth + 2, "name")?;
            value_json::write_json_string(out.sink, selected.field.name())?;
            out.key(false, depth + 2, "value")?;
            pending.push(Task::CloseWrapper(depth + 1));
            pending.push(Task::Value(selected.field.value(), depth + 2));
        } else {
            out.key(false, depth + 1, selected.field.name())?;
            pending.push(Task::Value(selected.field.value(), depth + 1));
        }
        Ok(())
    }
}
