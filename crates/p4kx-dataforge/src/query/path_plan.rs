// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::DataType;
use crate::query::reader::Reader;
use crate::query::resolve::{self, Resolved};
use crate::query::schema::{FieldDef, StructFields};
use crate::query::suggest;
use crate::query::syntax::{Name, Path};
use crate::query::{QueryError, QueryErrorKind};
use crate::source_layout_index;

/// The kind of a field that decides how the rest of a path resolves behind it.
type SlotKey = (DataType, u16, bool);

/// How one path of a Query is read from the Records of each struct.
#[derive(Debug, Clone)]
pub(crate) enum PathPlan {
    /// With `from`: resolved once against the `from` struct, for the structs whose first field
    /// of the path stands at `position` as it does there.
    Fixed {
        head: String,
        position: u64,
        resolved: Resolved,
    },
    /// Without `from`: resolved once per kind of field the path starts with, relative to it.
    ByHead {
        head: String,
        resolved: Vec<(SlotKey, Resolved)>,
    },
}

impl PathPlan {
    /// Resolves `path` against the struct at `nominal`, the `from` struct, or without `from`
    /// against each struct that declares its first field.
    ///
    /// Without `from`, the path is an error only when it resolves in no struct; the error is the
    /// one of the first struct that declares the field.
    pub(crate) fn of(
        structs: &StructFields<'_>,
        nominal: Option<usize>,
        path: &Path,
        from: Option<&Name>,
    ) -> Result<Self, QueryError> {
        let head = &path.head;
        let source_error = |error| QueryError::new(QueryErrorKind::DataForge(error), head.span());
        if let (Some(index), Some(from)) = (nominal, from) {
            return Self::fixed(structs, index, path, from);
        }
        let mut seen: Vec<SlotKey> = Vec::new();
        let mut resolved = Vec::new();
        let mut first_error = None;
        for index in 0..structs.len() {
            let field = match structs.field(index, head.as_str()) {
                Ok(Some(field)) => field,
                Err(error) if source_layout_index::is_operational(&error) => {
                    return Err(source_error(error));
                }
                Ok(None) | Err(_) => continue,
            };
            let key = key_of(&field);
            if seen.contains(&key) {
                continue;
            }
            seen.push(key);
            let relative = FieldDef {
                start: 0,
                end: field.end.saturating_sub(field.start),
                ..field
            };
            match resolve::resolve(structs, &relative, path, None) {
                Ok(done) => resolved.push((key, done)),
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if resolved.is_empty() {
            if let Some(error) = first_error {
                return Err(error);
            }
            let suggestion = suggest::closest_fallible(head.as_str(), structs.all_field_names())
                .map_err(source_error)?;
            return Err(unknown_field(head, None, suggestion));
        }
        Ok(Self::ByHead {
            head: head.as_str().to_owned(),
            resolved,
        })
    }

    /// Resolves one path against its nominal struct.
    fn fixed(
        structs: &StructFields<'_>,
        index: usize,
        path: &Path,
        from: &Name,
    ) -> Result<Self, QueryError> {
        let head = &path.head;
        let source_error = |error| QueryError::new(QueryErrorKind::DataForge(error), head.span());
        let Some(field) = structs.field(index, head.as_str()).map_err(source_error)? else {
            return Err(unknown_field(
                head,
                Some(from.as_str()),
                suggest::closest_fallible(
                    head.as_str(),
                    structs.field_names(index).map_err(source_error)?,
                )
                .map_err(source_error)?,
            ));
        };
        Ok(Self::Fixed {
            head: head.as_str().to_owned(),
            position: field.position,
            resolved: resolve::resolve(structs, &field, path, Some(from.as_str()))?,
        })
    }

    /// Returns the types of the scalars the path ends at, one per way it resolves.
    pub(crate) fn leaves(&self) -> impl Iterator<Item = DataType> + '_ {
        let (fixed, by_head) = match self {
            Self::Fixed { resolved, .. } => (Some(resolved.leaf), &[][..]),
            Self::ByHead { resolved, .. } => (None, &resolved[..]),
        };
        fixed
            .into_iter()
            .chain(by_head.iter().map(|(_, resolved)| resolved.leaf))
    }

    pub(crate) fn head(&self) -> &str {
        match self {
            Self::Fixed { head, .. } | Self::ByHead { head, .. } => head,
        }
    }

    /// Returns the reader for the actual first matching head occurrence.
    pub(crate) fn reader(&self, field: Option<&FieldDef<'_>>) -> Option<(Reader, DataType)> {
        let field = field?;
        match self {
            Self::Fixed {
                head,
                position,
                resolved,
            } => (field.name == head && field.position == *position)
                .then(|| (resolved.reader.clone(), resolved.leaf)),
            Self::ByHead { head, resolved } => {
                if field.name != head {
                    return None;
                }
                let key = key_of(field);
                resolved
                    .iter()
                    .find(|(candidate, _)| *candidate == key)
                    .map(|(_, done)| (done.reader.shifted(field.start), done.leaf))
            }
        }
    }
}

const fn key_of(field: &FieldDef<'_>) -> SlotKey {
    (
        field.slot.data_type(),
        field.slot.target(),
        field.slot.is_array(),
    )
}

fn unknown_field(name: &Name, owner: Option<&str>, suggestion: Option<String>) -> QueryError {
    QueryError::new(
        QueryErrorKind::UnknownField {
            name: name.as_str().to_owned(),
            owner: owner.map(str::to_owned),
            suggestion,
        },
        name.span(),
    )
}
