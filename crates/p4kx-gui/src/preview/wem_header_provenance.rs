// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::sound::{WemOriginError, WemOriginField, WemOriginPlan};
use p4kx_extraction::{
    FieldOrigin, InputScope, ObservedEntry, SourceEvidenceError, SourceInput, SourceInventory,
    SourceUses, Wem,
};
use thiserror::Error;

/// Why the original WEM metadata inputs could not be recorded.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WemHeaderEvidenceError {
    /// The original WEM decisions could not be mapped.
    #[error(transparent)]
    Decision(#[from] WemOriginError),
    /// The physical input or contribution could not be recorded.
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
}

/// Origins of the metadata shown for a directly opened WEM.
#[derive(Debug)]
#[non_exhaustive]
pub struct WemHeaderProvenance {
    /// Detected codec.
    pub codec: FieldOrigin,
    /// Declared channel count.
    pub channels: FieldOrigin,
    /// Effective playback rate in Hz.
    pub sample_rate: FieldOrigin,
    /// Playback duration or its absence.
    pub duration: FieldOrigin,
    inventory: SourceInventory,
}

impl WemHeaderProvenance {
    /// Returns the physical source and original field coordinates.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }

    pub(super) fn capture<B: AsRef<[u8]>>(
        wem: &Wem<B>,
        source: Option<ObservedEntry<'_>>,
    ) -> Result<Self, WemHeaderEvidenceError> {
        let uses = SourceUses::new();
        let complete = source
            .as_ref()
            .is_some_and(|entry| entry.ancestors().all(|step| step.is_some()));
        let scope = uses.observe(
            source.map_or(SourceInput::Unknown, SourceInput::ArchiveView),
            InputScope::Payload,
        )?;
        let plan = WemOriginPlan::new(wem.info_source(), wem.codec(), wem.sample_count());
        let field = |field| {
            let sites = plan.field(field)?;
            let inputs = sites
                .iter()
                .map(|site| site.bind(&scope))
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, WemHeaderEvidenceError>(uses.origin(&inputs, complete))
        };
        Ok(Self {
            codec: field(WemOriginField::Codec)?,
            channels: field(WemOriginField::Channels)?,
            sample_rate: field(WemOriginField::SampleRate)?,
            duration: field(WemOriginField::Duration)?,
            inventory: uses.freeze()?,
        })
    }
}
