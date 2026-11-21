// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::ChunkFile;

use crate::animation_dba;
use crate::{AnimationError, MAX_DATABASE_KEYS};

/// The complete declaration counts of a DBA header and its controller tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct DbaHeaderDeclarations {
    /// The number of declared clips.
    pub clips: u64,
    /// The number of declared rotation and translation keys.
    pub keys: u64,
    pub(crate) clips_over_preview_budget: u64,
}

impl DbaHeaderDeclarations {
    /// Reads and checks all DBA declarations without decoding keys or applying a key budget.
    ///
    /// # Errors
    /// Returns an error for invalid declared metadata, names, controller tables or counts.
    pub fn read<B: AsRef<[u8]>>(
        file_name: &str,
        file: &ChunkFile<B>,
    ) -> Result<Self, AnimationError> {
        animation_dba::read_header_projection(file_name, file, animation_dba::read_declarations)
    }

    /// Returns the number of budget refusals the preview header reader would report.
    #[must_use]
    pub const fn preview_budget_refusals(self) -> u64 {
        if self.keys > MAX_DATABASE_KEYS {
            1
        } else {
            self.clips_over_preview_budget
        }
    }
}
