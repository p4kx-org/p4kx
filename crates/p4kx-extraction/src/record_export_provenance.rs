// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    DataForgeOrigin, FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceInventory,
    SourceLocation, SourceUse, SourceUseRole, SourceUses,
};

/// The written Record count could not be bound to its original definitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RecordExportProvenanceError {
    /// The export did not retain the committed definition positions.
    #[error("the Record export did not retain committed definitions")]
    MissingDefinitions,
    /// The recorded definition positions do not account for the whole written count.
    #[error("the committed definition list does not match the exported Record count")]
    CountMismatch,
    /// The original source and use inventory could not be built.
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
}

/// Original inputs for the written Record count of one export.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecordExportProvenance {
    /// The original definitions of the Records actually committed by the run.
    pub written: FieldOrigin,
    inventory: SourceInventory,
}

impl RecordExportProvenance {
    /// Returns the physical inputs and original definition ranges.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }

    /// Takes the written count origin and its source inventory.
    #[must_use]
    pub fn into_parts(self) -> (FieldOrigin, SourceInventory) {
        (self.written, self.inventory)
    }

    /// Verifies the committed definition count and binds its original positions.
    ///
    /// # Errors
    ///
    /// Returns an error if accounting is missing, inconsistent, or cannot be bound.
    pub fn capture_count(
        source: Option<&DataForgeOrigin>,
        indices: Option<Vec<u32>>,
        written: usize,
    ) -> Result<Self, RecordExportProvenanceError> {
        let indices = indices.ok_or(RecordExportProvenanceError::MissingDefinitions)?;
        if indices.len() != written {
            return Err(RecordExportProvenanceError::CountMismatch);
        }
        Self::capture(source, indices).map_err(Into::into)
    }

    /// Binds the observed source to the committed definition indices from a Record export.
    ///
    /// # Errors
    /// Returns an error if the original use inventory cannot be recorded.
    pub fn capture(
        source: Option<&DataForgeOrigin>,
        mut indices: Vec<u32>,
    ) -> Result<Self, SourceEvidenceError> {
        let uses = SourceUses::new();
        let (written, complete) = if indices.is_empty() {
            (Vec::new(), true)
        } else {
            indices.sort_unstable();
            committed_uses(&uses, source, &indices)?
        };
        Ok(Self {
            written: uses.origin(&written, complete),
            inventory: uses.freeze()?,
        })
    }
}

fn committed_uses(
    uses: &SourceUses,
    source: Option<&DataForgeOrigin>,
    indices: &[u32],
) -> Result<(Vec<SourceUse>, bool), SourceEvidenceError> {
    let (input, complete) = match source {
        Some(DataForgeOrigin::Archive(entry)) => (
            SourceInput::Archive(entry),
            entry.ancestors().all(|step| step.is_some()),
        ),
        Some(DataForgeOrigin::File(file)) => (SourceInput::File(file), true),
        _ => (SourceInput::Unknown, false),
    };
    let scope = uses.observe(input, InputScope::Payload)?;
    let runs = indices.chunk_by(|first, next| first.checked_add(1) == Some(*next));
    let mut written = Vec::new();
    for run in runs {
        let (Some(first), Some(last)) = (run.first(), run.last()) else {
            continue;
        };
        written.push(scope.at(
            SourceLocation::Subresource {
                namespace: "dataforgeRecordTable".into(),
                indices: vec![u64::from(*first), u64::from(*last) + 1],
                field: "committedDefinitions".into(),
            },
            SourceUseRole::Conversion,
        )?);
    }
    Ok((written, complete))
}
