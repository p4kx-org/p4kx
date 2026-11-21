// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use super::tasks::{Fields, Head, Task};
use super::{classification, sink};
use crate::{Annotations, Field, RecordTree, Value};

/// The namespace of the `p4kx:` elements of Record XML.
pub const RECORD_XML_NAMESPACE: &str = "https://github.com/p4kx-org/p4kx/record-xml";

const DECLARATION: &[u8] = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";

/// Writes Record trees as Record XML, the format `docs/record-xml.md` defines.
///
/// One writer can be reused for many Records; it keeps its scratch space between them. The same
/// tree always gives the same bytes.
#[derive(Debug, Default)]
pub struct RecordXml {
    order: Vec<usize>,
    attribute: Vec<bool>,
}

impl RecordXml {
    /// Creates a writer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes `tree` as one Record XML document to `out`: the XML declaration, then the root
    /// element.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of `out`.
    pub fn write<W: Write>(&mut self, tree: &RecordTree<'_>, out: &mut W) -> io::Result<()> {
        self.write_annotated(tree, Annotations::none(), out)
    }

    /// Writes `tree` like [`RecordXml::write`] with the `annotations`: the texts of localisation
    /// keys, each with its resolved text or a mark that it has none, and the tag path of every
    /// tag Reference in a `tag` attribute, see `docs/record-xml.md`.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of `out`.
    pub fn write_annotated<W: Write>(
        &mut self,
        tree: &RecordTree<'_>,
        annotations: Annotations<'_>,
        out: &mut W,
    ) -> io::Result<()> {
        out.write_all(DECLARATION)?;
        self.write_element_annotated(tree, annotations, 0, out)
    }

    /// Writes the root element of `tree` without the XML declaration, indented by `depth` levels
    /// of two spaces, e.g. to place many Records in one document.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of `out`.
    pub fn write_element<W: Write>(
        &mut self,
        tree: &RecordTree<'_>,
        depth: usize,
        out: &mut W,
    ) -> io::Result<()> {
        self.write_element_annotated(tree, Annotations::none(), depth, out)
    }

    /// Writes the root element like [`RecordXml::write_element`], with the `annotations` of
    /// [`RecordXml::write_annotated`].
    ///
    /// # Errors
    ///
    /// Returns the I/O error of `out`.
    pub fn write_element_annotated<W: Write>(
        &mut self,
        tree: &RecordTree<'_>,
        annotations: Annotations<'_>,
        depth: usize,
        out: &mut W,
    ) -> io::Result<()> {
        self.attribute.clear();
        let head = Head::Record(tree, true);
        self.write_iterative(out, head, tree.fields(), depth, annotations)
    }

    /// Writes the root element of `tree` like [`RecordXml::write_element`], but without the
    /// `xmlns:p4kx` declaration, for a wrapper element that declares [`RECORD_XML_NAMESPACE`]
    /// itself.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of `out`.
    pub fn write_wrapped_element<W: Write>(
        &mut self,
        tree: &RecordTree<'_>,
        depth: usize,
        out: &mut W,
    ) -> io::Result<()> {
        self.write_wrapped_element_annotated(tree, Annotations::none(), depth, out)
    }

    /// Writes the wrapped root element like [`RecordXml::write_wrapped_element`], with the
    /// `annotations` of [`RecordXml::write_annotated`]; the wrapper declares the namespace.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of `out`.
    pub fn write_wrapped_element_annotated<W: Write>(
        &mut self,
        tree: &RecordTree<'_>,
        annotations: Annotations<'_>,
        depth: usize,
        out: &mut W,
    ) -> io::Result<()> {
        self.attribute.clear();
        let head = Head::Record(tree, false);
        self.write_iterative(out, head, tree.fields(), depth, annotations)
    }

    fn write_iterative<'tree, 'source>(
        &mut self,
        out: &mut impl Write,
        head: Head<'tree, 'source>,
        fields: &'tree [Field<'source>],
        depth: usize,
        annotations: Annotations<'_>,
    ) -> io::Result<()> {
        let mut pending = vec![Task::Structure {
            head,
            fields,
            depth,
        }];
        while let Some(task) = pending.pop() {
            self.step(out, task, annotations, &mut pending)?;
        }
        Ok(())
    }

    fn step<'tree, 'source>(
        &mut self,
        out: &mut impl Write,
        task: Task<'tree, 'source>,
        annotations: Annotations<'_>,
        pending: &mut Vec<Task<'tree, 'source>>,
    ) -> io::Result<()> {
        match task {
            Task::Structure {
                head,
                fields,
                depth,
            } => {
                self.structure(out, head, fields, depth, annotations, pending)?;
            }
            Task::Fields(fields) => self.fields(out, fields, pending)?,
            Task::Child(field, depth) => Self::child(out, field, depth, annotations, pending)?,
            Task::Value(value, depth) => Self::value(out, value, depth, annotations, pending)?,
            Task::Items(mut items, depth) => {
                if let Some(item) = items.next() {
                    pending.push(Task::Items(items, depth));
                    pending.push(Task::Value(item, depth));
                }
            }
            Task::CloseField { name, depth } => sink::close(out, name, depth)?,
        }
        Ok(())
    }

    fn structure<'tree, 'source>(
        &mut self,
        out: &mut impl Write,
        head: Head<'tree, 'source>,
        fields: &'tree [Field<'source>],
        depth: usize,
        annotations: Annotations<'_>,
        pending: &mut Vec<Task<'tree, 'source>>,
    ) -> io::Result<()> {
        let base = self.attribute.len();
        classification::classify(fields, &mut self.attribute, &mut self.order);
        let flags = self.attribute.get(base..).unwrap_or_default();
        if let Some(name) = sink::open(out, head, fields, flags, depth, annotations)? {
            pending.push(Task::Fields(Fields::new(fields, base, name, depth)));
        } else {
            self.attribute.truncate(base);
        }
        Ok(())
    }

    fn fields<'tree, 'source>(
        &mut self,
        out: &mut impl Write,
        mut fields: Fields<'tree, 'source>,
        pending: &mut Vec<Task<'tree, 'source>>,
    ) -> io::Result<()> {
        if let Some(field) = fields.next(&self.attribute) {
            let depth = fields.depth + 1;
            pending.push(Task::Fields(fields));
            pending.push(Task::Child(field, depth));
        } else {
            self.attribute.truncate(fields.base);
            sink::close(out, fields.name, fields.depth)?;
        }
        Ok(())
    }
    fn child<'tree, 'source>(
        out: &mut impl Write,
        field: &'tree Field<'source>,
        depth: usize,
        annotations: Annotations<'_>,
        pending: &mut Vec<Task<'tree, 'source>>,
    ) -> io::Result<()> {
        let value = field.value();
        if matches!(value, Value::Error(_)) || value.is_scalar() {
            return sink::field_leaf(out, field, depth, annotations);
        }
        let items = match value {
            Value::Array(items) => &items[..],
            single => std::slice::from_ref(single),
        };
        let name = sink::open_field(out, field.name(), items.is_empty(), depth)?;
        if !items.is_empty() {
            pending.push(Task::CloseField { name, depth });
            pending.push(Task::Items(items.iter(), depth + 1));
        }
        Ok(())
    }

    fn value<'tree, 'source>(
        out: &mut impl Write,
        value: &'tree Value<'source>,
        depth: usize,
        annotations: Annotations<'_>,
        pending: &mut Vec<Task<'tree, 'source>>,
    ) -> io::Result<()> {
        match value {
            Value::Struct(structure) => pending.push(Task::Structure {
                head: Head::Nested(structure.type_name()),
                fields: structure.fields(),
                depth,
            }),
            Value::Array(items) => pending.push(Task::Items(items.iter(), depth)),
            leaf => sink::value_leaf(out, leaf, depth, annotations)?,
        }
        Ok(())
    }
}

impl RecordTree<'_> {
    /// Writes the Record as one Record XML document to `out`.
    ///
    /// Use a [`RecordXml`] instead to write many Records with one scratch space.
    ///
    /// # Errors
    ///
    /// Returns the I/O error of `out`.
    pub fn write_xml<W: Write>(&self, out: &mut W) -> io::Result<()> {
        RecordXml::new().write(self, out)
    }
}
