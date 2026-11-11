// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, StructValue, Value};

use super::{FieldCapture, FieldError};
use crate::record_read::ReadError;

/// A failed actual guard before source labels or diagnostic positions are copied.
pub(crate) struct FieldViolation<'view, 'source> {
    parent: &'view StructValue<'source>,
    path: &'view str,
    name: &'view str,
    kind: ViolationKind<'view, 'source>,
}

enum ViolationKind<'view, 'source> {
    Invalid {
        value: &'view Value<'source>,
        expected: &'static str,
    },
    Unsupported {
        declaration: &'view Field<'source>,
    },
}

impl<'view, 'source> FieldViolation<'view, 'source> {
    pub(crate) const fn invalid(
        parent: &'view StructValue<'source>,
        path: &'view str,
        name: &'view str,
        value: &'view Value<'source>,
        expected: &'static str,
    ) -> Self {
        Self {
            parent,
            path,
            name,
            kind: ViolationKind::Invalid { value, expected },
        }
    }

    pub(crate) const fn unsupported(
        parent: &'view StructValue<'source>,
        path: &'view str,
        declaration: &'view Field<'source>,
    ) -> Self {
        Self {
            parent,
            path,
            name: declaration.name(),
            kind: ViolationKind::Unsupported { declaration },
        }
    }

    pub(crate) const fn parent(&self) -> &'view StructValue<'source> {
        self.parent
    }

    pub(crate) const fn value(&self) -> Option<&'view Value<'source>> {
        match self.kind {
            ViolationKind::Invalid { value, .. } => Some(value),
            ViolationKind::Unsupported { .. } => None,
        }
    }

    pub(crate) const fn declaration(&self) -> Option<&'view Field<'source>> {
        match self.kind {
            ViolationKind::Unsupported { declaration } => Some(declaration),
            ViolationKind::Invalid { .. } => None,
        }
    }

    pub(crate) fn field_text(&self) -> String {
        match (self.path.is_empty(), self.name.is_empty()) {
            (true, _) => self.name.to_owned(),
            (_, true) => self.path.to_owned(),
            _ => format!("{}.{}", self.path, self.name),
        }
    }

    pub(crate) fn captured_error(self, capture: &dyn FieldCapture<'source>) -> ReadError {
        match capture.field_failure(&self) {
            Ok(body) => Box::new(self.into_error().with_body(body)),
            Err(error) => Box::new(error),
        }
    }

    pub(crate) fn into_error(self) -> FieldError {
        let source_type = self.parent.type_name().to_owned();
        let field = self.field_text();
        match self.kind {
            ViolationKind::Invalid {
                value: Value::Error(source),
                ..
            } => FieldError::Unreadable {
                source_type,
                field,
                source: *source,
            },
            ViolationKind::Invalid { expected, .. } => FieldError::Malformed {
                source_type,
                field,
                expected,
            },
            ViolationKind::Unsupported { .. } => FieldError::Unsupported { source_type, field },
        }
    }
}
