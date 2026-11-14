// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Field, Record, RecordTree, SourceEvidenceError, Value};

use crate::{CatalogError, ExportAccountingError};

use super::frame::{Child, DeclarationSource, Frame, OwnerPosition};
use super::path::{OriginalPath, PathMark};
use super::tokens::{OriginalContainerKind, OriginalValueEdge};
use super::visit::OwnedValueVisit;

/// An actual value visit or completion of that value's owned container.
pub(crate) enum OwnedValueEvent<'visit, 'read, 'source> {
    Value(OwnedValueVisit<'visit, 'read, 'source>),
    EndContainer {
        ordinal: u64,
        kind: OriginalContainerKind,
    },
}

#[derive(Clone, Copy)]
pub(super) enum WalkFailure {
    Cancelled,
    CounterOverflow,
    FieldPath(std::fmt::Error),
}

impl WalkFailure {
    fn error(self) -> CatalogError {
        match self {
            Self::Cancelled => CatalogError::Cancelled,
            Self::CounterOverflow => ExportAccountingError::CounterOverflow.into(),
            Self::FieldPath(error) => ExportAccountingError::FieldPath(error).into(),
        }
    }
}

struct AfterValue<'read, 'source> {
    field: &'read Field<'source>,
    value: &'read Value<'source>,
    ordinal: u64,
    restore: PathMark,
    declaration_ordinal: u64,
    owner: OwnerPosition<'source>,
}

/// The single iterative DFS over materialized owned values, independent of source inventory.
pub(crate) struct OwnedValueWalk<'read, 'source, 'cancel> {
    cancel: Option<&'cancel CancelToken>,
    frames: Vec<Frame<'read, 'source>>,
    path: OriginalPath,
    after: Option<AfterValue<'read, 'source>>,
    next_ordinal: u64,
    failure: Option<WalkFailure>,
}

impl<'read, 'source, 'cancel> OwnedValueWalk<'read, 'source, 'cancel> {
    /// Walks the exact materialized original Record without minting source-use authority.
    pub(crate) fn new(
        record: &Record<'_>,
        tree: &'read RecordTree<'source>,
        cancel: Option<&'cancel CancelToken>,
    ) -> Result<Self, CatalogError> {
        if !tree.is_from(record) {
            return Err(SourceEvidenceError::ForeignEvidence.into());
        }
        Ok(Self::admitted(tree, cancel))
    }

    pub(super) fn admitted(
        tree: &'read RecordTree<'source>,
        cancel: Option<&'cancel CancelToken>,
    ) -> Self {
        let root = tree.root();
        Self {
            cancel,
            frames: vec![Frame::root(root)],
            path: OriginalPath::default(),
            after: None,
            next_ordinal: 0,
            failure: None,
        }
    }
}

impl<'read, 'source> OwnedValueWalk<'read, 'source, '_> {
    pub(crate) fn next_event(
        &mut self,
    ) -> Result<Option<OwnedValueEvent<'_, 'read, 'source>>, CatalogError> {
        if let Some(failure) = self.failure {
            return Err(failure.error());
        }
        if self.frames.is_empty() {
            return Ok(None);
        }
        if self.cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(self.stop(WalkFailure::Cancelled));
        }
        if let Some(after) = self.after.take() {
            self.descend(&after);
        }
        let Some(frame) = self.frames.last_mut() else {
            return Ok(None);
        };
        let child = frame.next_child().map_err(|failure| self.stop(failure))?;
        let Some(child) = child else {
            return Ok(self.end_container());
        };
        self.visit(&child)
    }

    fn visit(
        &mut self,
        child: &Child<'read, 'source>,
    ) -> Result<Option<OwnedValueEvent<'_, 'read, 'source>>, CatalogError> {
        let ordinal = self.next_ordinal;
        self.next_ordinal = ordinal
            .checked_add(1)
            .ok_or_else(|| self.stop(WalkFailure::CounterOverflow))?;
        let restore = self.extend_path(child)?;
        let declaration_ordinal = match child.declaration {
            DeclarationSource::Field => ordinal,
            DeclarationSource::Element { ordinal } => ordinal,
        };
        self.after = Some(AfterValue {
            field: child.field,
            value: child.value,
            ordinal,
            restore,
            declaration_ordinal,
            owner: child.owner,
        });
        Ok(Some(OwnedValueEvent::Value(OwnedValueVisit {
            field: child.field,
            value: child.value,
            ordinal,
            parent: child.parent,
            edge: child.edge,
            owner: child.owner,
            #[cfg(test)]
            frames: &self.frames,
            path: &self.path,
            declaration_ordinal,
        })))
    }

    fn extend_path(&mut self, child: &Child<'read, 'source>) -> Result<PathMark, CatalogError> {
        let restore = self.path.mark();
        match child.edge {
            OriginalValueEdge::Field { .. } => self.path.field(child.field.name()),
            OriginalValueEdge::Element { ordinal } => {
                self.path
                    .element(ordinal)
                    .map_err(|error| self.stop(WalkFailure::FieldPath(error)))?;
            }
        }
        Ok(restore)
    }

    fn descend(&mut self, after: &AfterValue<'read, 'source>) {
        match after.value {
            Value::Struct(source) => {
                self.frames.push(Frame::structure(
                    source,
                    Some(after.ordinal),
                    after.restore,
                    Some(after.owner.type_name),
                ));
            }
            Value::Array(values) => self.frames.push(Frame::array(
                after.field,
                values,
                after.ordinal,
                after.restore,
                after.owner,
                after.declaration_ordinal,
            )),
            _ => self.path.restore(after.restore),
        }
    }

    fn end_container(&mut self) -> Option<OwnedValueEvent<'_, 'read, 'source>> {
        let frame = self.frames.pop()?;
        self.path.restore(frame.restore);
        frame.ordinal.map(|ordinal| OwnedValueEvent::EndContainer {
            ordinal,
            kind: frame.kind(),
        })
    }

    fn stop(&mut self, failure: WalkFailure) -> CatalogError {
        self.failure = Some(failure);
        failure.error()
    }
}
