// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;
use std::io::{self, Write};

use super::observation::{FlatObserver, FlatRowObservation};
use super::task::{FlatTask, OriginalFlatFields};
use super::writer::{FlatValues, Leaf, Out};
use crate::{
    DataForgeError, Field, PositionedFields, PositionedValue, Value, ValueError, field_escapes,
};

struct Walk<'observer, 'tree, 'source, 'text> {
    pending: Vec<FlatTask<'tree, 'source>>,
    route: Vec<PositionedValue<'tree, 'source>>,
    aliases: Vec<PositionedValue<'tree, 'source>>,
    observer: Option<&'observer mut FlatObserver<'observer, 'tree, 'source, 'text>>,
    row_index: u64,
}

impl FlatValues {
    pub(super) fn walk_fields<'observer, 'tree, 'source, 'text, W: Write, const OBSERVED: bool>(
        &mut self,
        out: &mut Out<'_, 'text, W, OBSERVED>,
        fields: &'tree [Field<'source>],
        positions: Option<PositionedFields<'tree, 'source>>,
        observer: Option<&'observer mut FlatObserver<'observer, 'tree, 'source, 'text>>,
    ) -> io::Result<()> {
        let mut walk = Walk {
            pending: Vec::new(),
            route: Vec::new(),
            aliases: Vec::new(),
            observer,
            row_index: 0,
        };
        self.enter_fields(fields, positions, &mut walk.pending);
        let outcome = (|| {
            while let Some(task) = walk.pending.pop() {
                self.walk_step(out, task, &mut walk)?;
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
        positions: Option<PositionedFields<'tree, 'source>>,
        pending: &mut Vec<FlatTask<'tree, 'source>>,
    ) {
        let base = self.escaped.len();
        let mut aliases = positions.as_ref().map(|_| vec![None; fields.len()]);
        field_escapes::append_observed(
            fields,
            &mut self.escaped,
            &mut self.order,
            |first, second| {
                if let Some(previous) = aliases.as_mut().and_then(|aliases| aliases.get_mut(second))
                {
                    *previous = Some(first);
                }
            },
        );
        let positions = positions
            .zip(aliases)
            .map(|(slots, aliases)| OriginalFlatFields { slots, aliases });
        pending.push(FlatTask::Fields {
            fields,
            positions,
            next: 0,
            base,
        });
    }

    fn walk_step<'tree, 'source, 'text, W: Write, const OBSERVED: bool>(
        &mut self,
        out: &mut Out<'_, 'text, W, OBSERVED>,
        task: FlatTask<'tree, 'source>,
        walk: &mut Walk<'_, 'tree, 'source, 'text>,
    ) -> io::Result<()> {
        match task {
            FlatTask::Fields {
                fields,
                positions,
                next,
                base,
            } => self.walk_field(fields, positions, next, base, &mut walk.pending)?,
            FlatTask::Items {
                mut items,
                container,
            } => {
                if let Some((index, value)) = items.next() {
                    let original = selected(container, |container| container.element(index))?;
                    let mark = self.indices.len();
                    self.indices.push(index);
                    walk.pending.push(FlatTask::Items { items, container });
                    walk.pending.push(FlatTask::RestoreIndices(mark));
                    walk.pending.push(FlatTask::Value {
                        value,
                        original,
                        alias: None,
                    });
                }
            }
            FlatTask::Value {
                value,
                original,
                alias,
            } => {
                if let Some(original) = original {
                    walk.route.push(original);
                    let aliased = alias.is_some();
                    walk.aliases.extend(alias);
                    walk.pending.push(FlatTask::RestoreRoute { aliased });
                }
                self.walk_value(out, value, original, walk)?;
            }
            FlatTask::RestorePath(mark) => self.path.truncate(mark),
            FlatTask::RestoreIndices(mark) => self.indices.truncate(mark),
            FlatTask::RestoreRoute { aliased } => {
                let _original = recover(walk.route.pop())?;
                if aliased {
                    let _alias = recover(walk.aliases.pop())?;
                }
            }
        }
        Ok(())
    }

    fn walk_field<'tree, 'source>(
        &mut self,
        fields: &'tree [Field<'source>],
        positions: Option<OriginalFlatFields<'tree, 'source>>,
        next: usize,
        base: usize,
        pending: &mut Vec<FlatTask<'tree, 'source>>,
    ) -> io::Result<()> {
        let Some(field) = fields.get(next) else {
            self.escaped.truncate(base);
            return Ok(());
        };
        let original = selected(positions.as_ref(), |positions| positions.slots.get(next))?;
        let alias = match positions.as_ref() {
            Some(positions) => {
                let previous = positions.aliases.get(next).copied().flatten();
                selected(previous, |previous| positions.slots.get(previous))?
            }
            None => None,
        };
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
            positions,
            next: next + 1,
            base,
        });
        pending.push(FlatTask::RestorePath(mark));
        pending.push(FlatTask::Value {
            value: field.value(),
            original,
            alias,
        });
        Ok(())
    }

    fn walk_value<'tree, 'source, 'text, W: Write, const OBSERVED: bool>(
        &mut self,
        out: &mut Out<'_, 'text, W, OBSERVED>,
        value: &'tree Value<'source>,
        original: Option<PositionedValue<'tree, 'source>>,
        walk: &mut Walk<'_, 'tree, 'source, 'text>,
    ) -> io::Result<()> {
        let leaf = match value {
            Value::Struct(structure) => {
                let positions = original
                    .map(|original| {
                        original
                            .fields()
                            .map_err(io::Error::other)
                            .and_then(recover)
                    })
                    .transpose()?;
                self.enter_fields(structure.fields(), positions, &mut walk.pending);
                return Ok(());
            }
            Value::Array(items) => {
                walk.pending.push(FlatTask::Items {
                    items: (0..u32::MAX).zip(items.iter()),
                    container: original,
                });
                return Ok(());
            }
            Value::Error(ValueError::DanglingReference { guid }) => Leaf::Dangling(*guid),
            Value::Null | Value::Error(_) => return Ok(()),
            Value::String(text) | Value::Locale(text) | Value::Enum(text) if text.is_empty() => {
                return Ok(());
            }
            leaf => Leaf::Value(leaf),
        };
        let localization = self.write_row(out, leaf)?;
        if let Some(observer) = walk.observer.as_mut() {
            observer(FlatRowObservation {
                row_index: walk.row_index,
                field_path: &self.path,
                array_indices: &self.indices,
                columns: out.spans(),
                route: &walk.route,
                field_path_aliases: &walk.aliases,
                localization: localization.as_ref(),
            })?;
            walk.row_index = walk
                .row_index
                .checked_add(1)
                .ok_or_else(|| io::Error::other("flat row count exceeds u64"))?;
        }
        Ok(())
    }
}

fn selected<T, U>(parent: Option<T>, find: impl FnOnce(T) -> Option<U>) -> io::Result<Option<U>> {
    parent.map(|parent| recover(find(parent))).transpose()
}

fn recover<T>(position: Option<T>) -> io::Result<T> {
    position.ok_or_else(|| io::Error::other(DataForgeError::CanonicalTreeInvariant))
}
