// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForgeOrigin, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath, FieldPathError,
    InputScope, SourceEvidenceError, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses,
};
use thiserror::Error;

use crate::forge_shapes::ForgeFieldOrigins;

#[derive(Debug, Error)]
pub(crate) enum EvidenceError {
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
    #[error(transparent)]
    Path(#[from] FieldPathError),
    #[error(transparent)]
    Duplicate(#[from] FieldOriginsError),
}

pub(crate) struct ForgeOrigins {
    pub(crate) uses: SourceUses,
    pub(crate) scope: SourceScope,
    fields: FieldOrigins,
    pub(crate) complete: bool,
}

impl ForgeOrigins {
    pub(crate) fn new(origin: &DataForgeOrigin) -> Result<Self, SourceEvidenceError> {
        let (source, complete) = match origin {
            DataForgeOrigin::Archive(entry) => (
                SourceInput::Archive(entry),
                entry.ancestors().all(|step| step.is_some()),
            ),
            DataForgeOrigin::File(file) => (SourceInput::File(file), true),
            _ => (SourceInput::Unknown, false),
        };
        let uses = SourceUses::with_compact_use_refs();
        let scope = uses.observe(source, InputScope::Payload)?;
        Ok(Self {
            uses,
            scope,
            fields: FieldOrigins::new(),
            complete,
        })
    }

    pub(crate) fn put(&mut self, pointer: &str, origin: FieldOrigin) -> Result<(), EvidenceError> {
        self.fields.insert(
            FieldPath::parse(&format!("/structuredContent/{pointer}"))?,
            origin,
        )?;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<ForgeFieldOrigins, EvidenceError> {
        Ok(ForgeFieldOrigins {
            fields: self.fields,
            provenance: self.uses.freeze()?,
        })
    }
}

pub(crate) fn site(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    indices: Vec<u64>,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            indices,
            field: field.into(),
        },
        role,
    )
}
