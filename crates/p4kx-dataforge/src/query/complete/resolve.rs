// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::context::{Spot, Step, Trigger, Walk, Want};
use super::index::{Field, QuerySchema, by_name};
use super::{At, Candidate, Completion, CompletionKind, MAX_COMPLETIONS};
use crate::DataType;
use crate::query::lexer::is_bare_name;

/// A name that fits, with the field it names, `None` for a struct name.
type Named = (usize, Option<Field>);

impl QuerySchema {
    /// Returns the names that fit at byte `cursor` of `text`, or `None` where no type or field
    /// name fits, the text before the cursor cannot be read or nothing starts with the typed
    /// word.
    #[must_use]
    pub fn complete(&self, text: &str, cursor: usize) -> Option<Completion> {
        let spot = super::context::spot(text, cursor)?;
        let (kind, names) = match &spot.want {
            Want::Type => (
                CompletionKind::Type,
                self.types.iter().map(|&id| (id, None)).collect(),
            ),
            Want::Head => (CompletionKind::Field, self.heads(spot.from)),
            Want::Behind { path, trigger } => self.behind(spot.from, path, *trigger)?,
        };
        self.listed(&spot, kind, &names)
    }

    fn listed(&self, spot: &Spot<'_>, kind: CompletionKind, names: &[Named]) -> Option<Completion> {
        let mut candidates = Vec::new();
        let mut more = 0_usize;
        for &(id, field) in names {
            let name = self.name(id);
            if name.contains('`') || !starts_with_ignoring_case(name, spot.prefix) {
                continue;
            }
            if candidates.len() == MAX_COMPLETIONS {
                more = more.saturating_add(1);
                continue;
            }
            candidates.push(Candidate {
                name: name.to_owned(),
                insert: if is_bare_name(name) {
                    name.to_owned()
                } else {
                    format!("`{name}`")
                },
                detail: field.map(|field| self.describe(field)),
            });
        }
        (!candidates.is_empty()).then(|| Completion {
            replace: spot.replace.clone(),
            kind,
            candidates,
            more,
        })
    }

    /// Returns the first fields of a path: those of the `from` type, or of every struct.
    fn heads(&self, from: Option<&str>) -> Vec<Named> {
        from.map_or_else(
            || {
                self.all_fields
                    .iter()
                    .map(|field| (field.name, Some(*field)))
                    .collect()
            },
            |from| {
                self.position_of(from)
                    .map_or_else(Vec::new, |index| self.fields_of(&[index]))
            },
        )
    }

    /// Returns the fields of the structs at `indices`, each name once, in name order.
    fn fields_of(&self, indices: &[usize]) -> Vec<Named> {
        let mut selected = std::collections::HashMap::new();
        for &index in indices {
            let mut own = std::collections::HashMap::new();
            for field in self.fields(index) {
                own.insert(field.name, field);
            }
            for (name, field) in own {
                selected.entry(name).or_insert(field);
            }
        }
        let mut names: Vec<Named> = selected
            .into_iter()
            .map(|(name, field)| (name, Some(field)))
            .collect();
        names.sort_unstable_by(|a, b| by_name(self.name(a.0), self.name(b.0)));
        names
    }

    fn behind(
        &self,
        from: Option<&str>,
        path: &Walk<'_>,
        trigger: Trigger,
    ) -> Option<(CompletionKind, Vec<Named>)> {
        let ends = self.ends(from, path);
        let mut targets: Vec<usize> = ends
            .iter()
            .filter_map(|at| match (at, trigger) {
                (
                    At::Array {
                        data_type: DataType::Class | DataType::StrongPointer,
                        target,
                    },
                    Trigger::Bracket,
                ) => Some(usize::from(*target)),
                (At::Struct(index), Trigger::Dot) | (At::Link(index), Trigger::Arrow) => {
                    Some(*index)
                }
                _ => None,
            })
            .filter(|&index| index < self.len())
            .collect();
        targets.sort_unstable();
        targets.dedup();
        if targets.is_empty() {
            return None;
        }
        Some(if trigger == Trigger::Bracket {
            (CompletionKind::Type, self.types_under(&targets))
        } else {
            (CompletionKind::Field, self.fields_of(&targets))
        })
    }

    /// Returns the struct names of the structs that are one of `bases`, derive from one or are
    /// one of their bases: every type a component selector accepts.
    fn types_under(&self, bases: &[usize]) -> Vec<Named> {
        let mut ids: Vec<usize> = (0..self.len())
            .filter(|&index| bases.iter().any(|&base| self.related(index, base)))
            .filter_map(|index| self.entry(index).and_then(|entry| entry.name))
            .collect();
        ids.sort_unstable_by(|a, b| by_name(self.name(*a), self.name(*b)));
        ids.dedup();
        ids.into_iter().map(|id| (id, None)).collect()
    }

    /// Returns where `path` ends in the `from` type, or in each struct declaring its first
    /// field.
    fn ends(&self, from: Option<&str>, path: &Walk<'_>) -> Vec<At> {
        let starts: Vec<usize> = from.map_or_else(
            || self.declaring(path.head).collect(),
            |from| self.position_of(from).into_iter().collect(),
        );
        starts
            .into_iter()
            .filter_map(|start| {
                let field = self.field(start, path.head)?;
                path.steps
                    .iter()
                    .try_fold(enter(field), |at, step| self.step(at, *step))
            })
            .collect()
    }

    fn step(&self, at: At, step: Step<'_>) -> Option<At> {
        match (at, step) {
            (At::Struct(index), Step::Field(name)) | (At::Link(index), Step::Follow(name)) => {
                self.field(index, name).map(enter)
            }
            (At::Array { data_type, target }, Step::Elements) => Some(element(data_type, target)),
            (
                At::Array {
                    data_type: DataType::Class | DataType::StrongPointer,
                    target,
                },
                Step::Type(name),
            ) => self
                .position_of(name)
                .filter(|&index| self.related(index, usize::from(target)))
                .map(At::Struct),
            _ => None,
        }
    }

    fn describe(&self, field: Field) -> String {
        let element = match field.data_type {
            DataType::Class
            | DataType::StrongPointer
            | DataType::WeakPointer
            | DataType::Reference => self.struct_name(usize::from(field.target)).map_or_else(
                || field.data_type.to_string(),
                |target| format!("{} {target}", field.data_type),
            ),
            scalar => scalar.to_string(),
        };
        if field.array {
            format!("{element}[]")
        } else {
            element
        }
    }
}

fn enter(field: Field) -> At {
    if field.array {
        return At::Array {
            data_type: field.data_type,
            target: field.target,
        };
    }
    element(field.data_type, field.target)
}

fn element(data_type: DataType, target: u16) -> At {
    match data_type {
        DataType::Class | DataType::StrongPointer => At::Struct(usize::from(target)),
        DataType::WeakPointer | DataType::Reference => At::Link(usize::from(target)),
        _ => At::Scalar,
    }
}

fn starts_with_ignoring_case(name: &str, prefix: &str) -> bool {
    name.get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}
