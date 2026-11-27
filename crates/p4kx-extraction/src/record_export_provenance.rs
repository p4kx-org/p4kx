// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    DataForgeOrigin, FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceInventory,
    SourceLocation, SourceUse, SourceUseRole, SourceUses,
};

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
