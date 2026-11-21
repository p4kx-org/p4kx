// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveInputOrigin, BuildManifestOutcome, SourceUses};

use crate::build_projection;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter, RootSource};

/// Original read authority plus a token that only checks an already-bound value.
#[derive(Debug, Clone)]
pub(crate) struct RootBuildOrigin {
    pub(crate) owner: SourceUses,
    pub(crate) root: ArchiveInputOrigin,
    pub(crate) input: Option<MediaOriginInput>,
    pub(crate) outcome: BuildManifestOutcome,
    pub(crate) fingerprint: Option<[u8; 32]>,
}

impl RootBuildOrigin {
    pub(crate) fn accepts(
        &self,
        root: &RootSource,
        writer: &MediaWriter<'_>,
    ) -> Result<bool, MediaError> {
        if !self.owner.is_same_inventory(&writer.origins.owner)
            || !self.root.is_same_input(&writer.origins.observer()?.root)
        {
            return Ok(false);
        }
        let Some(expected) = self.fingerprint else {
            return Ok(false);
        };
        Ok(build_projection::fingerprint(
            root.build.as_ref(),
            writer.limits().record_bytes,
            writer.checkpoint,
        )?
        .is_some_and(|actual| actual == expected))
    }
}
