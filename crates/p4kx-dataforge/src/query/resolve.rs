// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod follow;

use std::sync::Arc;

use crate::query::reader::{Column, Elements, Reader, Select, Step, Walk};
use crate::query::schema::{FieldDef, StructFields, ancestors_include, is_scalar};
use crate::query::suggest::{self, closest};
use crate::query::syntax::{Name, Path, Segment};
use crate::query::{QueryError, QueryErrorKind, Span};
use crate::{DataType, ValueKind};

/// A path checked against the schema: how its values are read and the type of its scalars.
#[derive(Debug, Clone)]
pub(crate) struct Resolved {
    pub(crate) reader: Reader,
    pub(crate) leaf: DataType,
}

/// What the part of a path read so far holds.
#[derive(Debug, Clone, Copy)]
enum At {
    /// A struct at the current offset, reached as `holds`: an inline class or a strong pointer.
    Struct { index: usize, holds: DataType },
    /// A strong pointer at the current offset, to an instance of `target` or a struct deriving
    /// from it.
    Pointer { target: usize },
    /// An array header at the current offset, with elements of `data_type`.
    Array { data_type: DataType, target: u16 },
    /// A Reference or weak pointer at the current offset, declared to link to `target`.
    Link { data_type: DataType, target: u16 },
    /// A scalar of `size` bytes at the current offset.
    Scalar {
        data_type: DataType,
        target: u16,
        size: u64,
    },
}

/// The state of resolving a path: the steps so far, where the path stands and how errors name
/// it.
struct Resolver<'s, 'f> {
    structs: &'s StructFields<'f>,
    steps: Vec<Step>,
    offset: u64,
    at: At,
    name: String,
    owner: Option<String>,
}

/// Resolves `path` from its first field `head`, which `owner` declares; offsets are relative
/// to the instance `head` lies in.
pub(crate) fn resolve(
    structs: &StructFields<'_>,
    head: &FieldDef<'_>,
    path: &Path,
    owner: Option<&str>,
) -> Result<Resolved, QueryError> {
    let mut resolver = Resolver {
        structs,
        steps: Vec::new(),
        offset: head.start,
        at: At::Scalar {
            data_type: DataType::Bool,
            target: 0,
            size: 0,
        },
        name: path.head.as_str().to_owned(),
        owner: owner.map(str::to_owned),
    };
    resolver.enter(head);
    for segment in &path.tail {
        match segment {
            Segment::Field(name) => resolver.field(name)?,
            Segment::Index { index, span } => resolver.elements(Select::Index(*index), *span)?,
            Segment::All(span) => resolver.elements(Select::All, *span)?,
            Segment::Type { name, span } => resolver.typed(name, *span)?,
            Segment::Follow { arrow, name } => resolver.follow(*arrow, name)?,
        }
        if !matches!(segment, Segment::Field(_) | Segment::Follow { .. }) {
            resolver.name.push_str(&segment.written());
        }
    }
    let span = match path.tail.last() {
        Some(Segment::Follow { name, .. }) => name.span,
        Some(segment) => segment.span(),
        None => path.head.span,
    };
    resolver.finish(span, path.is_multi_valued())
}

impl Resolver<'_, '_> {
    /// Moves onto `field`, whose offset is already added.
    fn enter(&mut self, field: &FieldDef<'_>) {
        let data_type = field.slot.data_type();
        let target = field.slot.target();
        self.at = if field.slot.is_array() {
            At::Array { data_type, target }
        } else {
            match data_type {
                DataType::Class => At::Struct {
                    index: usize::from(target),
                    holds: data_type,
                },
                DataType::StrongPointer => At::Pointer {
                    target: usize::from(target),
                },
                DataType::WeakPointer | DataType::Reference => At::Link { data_type, target },
                _ => At::Scalar {
                    data_type,
                    target,
                    size: field.end.saturating_sub(field.start),
                },
            }
        };
    }

    fn error(
        &self,
        kind: impl FnOnce(String, Option<String>) -> QueryErrorKind,
        span: Span,
    ) -> QueryError {
        QueryError::new(kind(self.name.clone(), self.owner.clone()), span)
    }

    /// Returns the struct whose fields `.next` names, leaving a strong pointer on the way, or
    /// the error for a value that has no fields.
    fn struct_for(&mut self, next: &Name) -> Result<usize, QueryError> {
        type NoFields = fn(String, Option<String>, DataType, String) -> QueryErrorKind;
        let (data_type, kind): (DataType, NoFields) = match self.at {
            At::Struct { index, .. } => return Ok(index),
            At::Pointer { target } => {
                self.leave(Step::Pointer {
                    at: self.offset,
                    declared: Box::new([target]),
                });
                return Ok(target);
            }
            At::Scalar { data_type, .. } => (data_type, |name, owner, data_type, next| {
                QueryErrorKind::ScalarHasNoFields {
                    name,
                    owner,
                    data_type,
                    next,
                }
            }),
            At::Array { data_type, .. } => (data_type, |name, owner, data_type, next| {
                QueryErrorKind::ArrayHasNoFields {
                    name,
                    owner,
                    data_type,
                    next,
                }
            }),
            At::Link { data_type, .. } => (data_type, |name, owner, data_type, next| {
                QueryErrorKind::LinkHasNoFields {
                    name,
                    owner,
                    data_type,
                    next,
                }
            }),
        };
        let next_name = next.as_str().to_owned();
        Err(self.error(
            |name, owner| kind(name, owner, data_type, next_name),
            next.span,
        ))
    }

    fn field(&mut self, next: &Name) -> Result<(), QueryError> {
        let index = self.struct_for(next)?;
        let structs = self.structs;
        let owner = structs.name(index).map(str::to_owned);
        let Some(field) = structs
            .field(index, next.as_str())
            .map_err(|error| QueryError::new(QueryErrorKind::DataForge(error), next.span))?
        else {
            return Err(QueryError::new(
                QueryErrorKind::UnknownField {
                    name: next.as_str().to_owned(),
                    owner,
                    suggestion: suggest::closest_fallible(
                        next.as_str(),
                        structs.field_names(index).map_err(|error| {
                            QueryError::new(QueryErrorKind::DataForge(error), next.span)
                        })?,
                    )
                    .map_err(|error| {
                        QueryError::new(QueryErrorKind::DataForge(error), next.span)
                    })?,
                },
                next.span,
            ));
        };
        self.offset = self.offset.saturating_add(field.start);
        next.as_str().clone_into(&mut self.name);
        self.owner = owner;
        self.enter(&field);
        Ok(())
    }

    /// Ends the bytes reached so far with `step`; the offsets after it start at 0.
    fn leave(&mut self, step: Step) {
        self.steps.push(step);
        self.offset = 0;
    }

    /// Selects elements by index or all of them.
    fn elements(&mut self, select: Select, span: Span) -> Result<(), QueryError> {
        let At::Array { data_type, target } = self.at else {
            return Err(self.not_an_array(span));
        };
        let (elements, at) = match ValueKind::of(data_type) {
            None => (
                Elements::Instances(u32::from(target)),
                At::Struct {
                    index: usize::from(target),
                    holds: data_type,
                },
            ),
            Some(ValueKind::StrongPointer) => (
                Elements::Pointers(Box::new([usize::from(target)])),
                At::Struct {
                    index: usize::from(target),
                    holds: data_type,
                },
            ),
            Some(kind) if is_scalar(data_type) => (
                Elements::Values(kind),
                At::Scalar {
                    data_type,
                    target,
                    size: kind.element_len() as u64,
                },
            ),
            Some(kind) => (Elements::Values(kind), At::Link { data_type, target }),
        };
        self.leave(Step::Array {
            at: self.offset,
            elements,
            select,
        });
        self.at = at;
        Ok(())
    }

    /// Selects the elements of a struct array whose struct is `type_name` or derives from it.
    fn typed(&mut self, type_name: &Name, span: Span) -> Result<(), QueryError> {
        let At::Array { data_type, target } = self.at else {
            return Err(self.not_an_array(span));
        };
        if !matches!(data_type, DataType::Class | DataType::StrongPointer) {
            return Err(self.error(
                |name, owner| QueryErrorKind::ElementsHaveNoType {
                    name,
                    owner,
                    data_type,
                },
                span,
            ));
        }
        let structs = self.structs;
        if structs.position_of(type_name.as_str()).is_none() {
            return Err(QueryError::new(
                QueryErrorKind::UnknownType {
                    name: type_name.as_str().to_owned(),
                    suggestion: closest(type_name.as_str(), structs.names()),
                },
                type_name.span,
            ));
        }
        let element = usize::from(target);
        let forge = structs.forge();
        let related = structs.positions_of(type_name.as_str()).find(|&index| {
            ancestors_include(forge, index, &[element])
                || ancestors_include(forge, element, &[index])
        });
        let Some(wanted) = related else {
            return Err(QueryError::new(
                QueryErrorKind::UnrelatedType {
                    name: self.name.clone(),
                    element: structs.name(element).unwrap_or_default().to_owned(),
                    type_name: type_name.as_str().to_owned(),
                },
                type_name.span,
            ));
        };
        let elements = if data_type == DataType::Class {
            Elements::Instances(u32::from(target))
        } else {
            Elements::Pointers(Box::new([element]))
        };
        self.leave(Step::Array {
            at: self.offset,
            elements,
            select: Select::Type(Box::new([wanted])),
        });
        self.at = At::Struct {
            index: wanted,
            holds: data_type,
        };
        Ok(())
    }

    fn not_an_array(&self, span: Span) -> QueryError {
        let data_type = match self.at {
            At::Struct { holds, .. } => holds,
            At::Pointer { .. } => DataType::StrongPointer,
            At::Link { data_type, .. }
            | At::Scalar { data_type, .. }
            | At::Array { data_type, .. } => data_type,
        };
        self.error(
            |name, owner| QueryErrorKind::NotAnArray {
                name,
                owner,
                data_type,
            },
            span,
        )
    }

    /// Returns the reader of the scalar the path ends at.
    fn finish(self, span: Span, multi: bool) -> Result<Resolved, QueryError> {
        let (data_type, array) = match self.at {
            At::Scalar {
                data_type,
                target,
                size,
            } => {
                let leaf = Column {
                    data_type,
                    target,
                    start: self.offset,
                    end: self.offset.saturating_add(size),
                };
                let reader = if self.steps.is_empty() {
                    Reader::Column(leaf)
                } else {
                    Reader::Walk(Arc::new(Walk {
                        steps: self.steps.into_boxed_slice(),
                        leaf,
                        multi,
                    }))
                };
                return Ok(Resolved {
                    reader,
                    leaf: data_type,
                });
            }
            At::Struct { holds, .. } => (holds, false),
            At::Pointer { .. } => (DataType::StrongPointer, false),
            At::Link { data_type, .. } => (data_type, false),
            At::Array { data_type, .. } => (data_type, true),
        };
        Err(self.error(
            |name, owner| QueryErrorKind::UnsupportedField {
                name,
                owner,
                data_type,
                array,
            },
            span,
        ))
    }
}
