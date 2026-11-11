// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;
use std::io::{self, Write};

use super::task::FlatTask;
use super::writer::{FlatValues, Leaf, Out};
use crate::{Field, Value, ValueError, field_escapes};

impl FlatValues {
    pub(super) fn walk_fields<W: Write>(
        &mut self,
        out: &mut Out<'_, '_, W>,
        fields: &[Field<'_>],
    ) -> io::Result<()> {
        let mut pending = Vec::new();
        self.enter_fields(fields, &mut pending);
        let outcome = (|| {
            while let Some(task) = pending.pop() {
                self.walk_step(out, task, &mut pending)?;
            }
            Ok(())
        })();
        if outcome.is_err() {
            self.path.clear();
            self.indices.clear();
            self.escaped.clear();
        }
        outcome
    }

    fn enter_fields<'tree, 'source>(
        &mut self,
        fields: &'tree [Field<'source>],
        pending: &mut Vec<FlatTask<'tree, 'source>>,
    ) {
        let base = self.escaped.len();
        field_escapes::append(fields, &mut self.escaped, &mut self.order);
        pending.push(FlatTask::Fields {
            fields,
            next: 0,
            base,
        });
    }

    fn walk_step<'tree, 'source, W: Write>(
        &mut self,
        out: &mut Out<'_, '_, W>,
        task: FlatTask<'tree, 'source>,
        pending: &mut Vec<FlatTask<'tree, 'source>>,
    ) -> io::Result<()> {
        match task {
            FlatTask::Fields { fields, next, base } => {
                if let Some(field) = fields.get(next) {
                    let mark = self.path.len();
                    if mark > 0 {
                        self.path.push('.');
                    }
                    if self.escaped.get(base + next) == Some(&true) {
                        write!(self.path, "${next}").map_err(io::Error::other)?;
                    } else {
                        self.path.push_str(field.name());
                    }
                    pending.push(FlatTask::Fields {
                        fields,
                        next: next + 1,
                        base,
                    });
                    pending.push(FlatTask::RestorePath(mark));
                    pending.push(FlatTask::Value(field.value()));
                } else {
                    self.escaped.truncate(base);
                }
            }
            FlatTask::Items(mut items) => {
                if let Some((index, value)) = items.next() {
                    let mark = self.indices.len();
                    self.indices.push(index);
                    pending.push(FlatTask::Items(items));
                    pending.push(FlatTask::RestoreIndices(mark));
                    pending.push(FlatTask::Value(value));
                }
            }
            FlatTask::Value(value) => self.walk_value(out, value, pending)?,
            FlatTask::RestorePath(mark) => self.path.truncate(mark),
            FlatTask::RestoreIndices(mark) => self.indices.truncate(mark),
        }
        Ok(())
    }

    fn walk_value<'tree, 'source, W: Write>(
        &mut self,
        out: &mut Out<'_, '_, W>,
        value: &'tree Value<'source>,
        pending: &mut Vec<FlatTask<'tree, 'source>>,
    ) -> io::Result<()> {
        match value {
            Value::Struct(structure) => self.enter_fields(structure.fields(), pending),
            Value::Array(items) => pending.push(FlatTask::Items((0..u32::MAX).zip(items.iter()))),
            Value::Error(ValueError::DanglingReference { guid }) => {
                return self.write_row(out, Leaf::Dangling(*guid));
            }
            Value::Null | Value::Error(_) => {}
            Value::String(text) | Value::Locale(text) | Value::Enum(text) if text.is_empty() => {}
            leaf => return self.write_row(out, Leaf::Value(leaf)),
        }
        Ok(())
    }
}
